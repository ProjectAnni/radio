mod repo;

use std::collections::VecDeque;
use std::env;
use std::process::Stdio;
use std::sync::Arc;
use std::io::Write;
use tokio::process::{ChildStdout, Command};
use anni_provider::{AnniProvider, ResourceReader, AudioResourceReader, Range};
use anni_provider::providers::ProxyBackend;
use anni_provider::cache::{Cache, CachePool};
use tempfile::{NamedTempFile, TempPath};
use tokio::fs::File;
use crate::repo::{RepoManager, TrackRef};

async fn to_s16le(mut reader: ResourceReader) -> anyhow::Result<ChildStdout> {
    let mut cmd = Command::new("ffmpeg")
        .args([
            // "-re",
            "-f", "flac",
            "-i", "pipe:0",
            "-f", "s16le",
            "-ac", "2",
            "-ar", "44100",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = cmd.stdin.take().expect("Failed to take stdin of ffmpeg(to_s16le)");
    tokio::spawn(async move {
        let _ = tokio::io::copy(&mut reader, &mut stdin).await;
        drop(stdin);
    });
    let stdout = cmd.stdout.take().expect("Failed to take stdout of ffmpeg(to_s16le)");
    Ok(stdout)
}

async fn generate_cover(provider: Arc<impl AnniProvider>, TrackRef { album_id, disc_id, track_id, album, track }: &TrackRef<'_, '_>) -> anyhow::Result<(AudioResourceReader, ChildStdout)> {
    let audio = provider.get_audio(&album_id, *disc_id as u8, *track_id as u8, Range::FULL).await?;
    let mut cover = provider.get_cover(&album_id, None).await?;

    // TODO: i18n support for text
    let text_temp_file = tempfile::NamedTempFile::new()?;
    let mut text_file = text_temp_file.as_file();
    write!(text_file, r#"
专辑 ID：{}
序号：{}/{}
标题：{}
艺术家：{}
专辑：{}
"#, album_id, album.catalog(), track_id, track.title(), track.artist(), album.title())?;
    let text_path = text_temp_file.path();
    let text_path = text_path.to_string_lossy();

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "color=c=black:s=1920x1080",
            "-i", "pipe:0",
            "-frames:v", "1",
            "-filter_complex", &format!("[1:v]scale=-1:'min(1000,ih)'[ovrl],
    [0:v][ovrl]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2,
    drawtext=
      fontfile=/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc:
      textfile={}:
      x=8: y='main_h-text_h-8':
      fontcolor=white:
      borderw=2:
      fontsize=24", text_path),
            "-f", "image2pipe",
            "-c:v", "png",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to generate image");

    let mut stdin = child.stdin.take().expect("Failed to take stdin of ffmpeg(cover)");
    let stdout = child.stdout.take().unwrap();
    tokio::io::copy(&mut cover, &mut stdin).await.expect("Failed to copy cover from reader");

    tokio::spawn(async move {
        // drop text file after child
        let file = text_temp_file;
        let _ = child.wait().await;
        drop(file);
    });

    Ok((audio, stdout))
}

async fn save_cover(mut cover: ChildStdout) -> anyhow::Result<TempPath> {
    let file = NamedTempFile::new()?;
    let (file, path) = file.into_parts();
    let mut file = File::from_std(file);
    tokio::io::copy(&mut cover, &mut file).await?;
    Ok(path)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = RepoManager::new(env::var("ANNI_REPO")?);
    let provider = ProxyBackend::new(env::var("ANNIL_URL")?, env::var("ANNIL_AUTH")?);
    let pool = Arc::new(CachePool::new("/tmp", 0));
    let provider = Cache::new(Box::new(provider), pool);
    let provider = Arc::new(provider);
    let albums = provider.albums().await?;

    let output = env::args().nth(1).unwrap_or(String::from("-"));
    let mut args = vec![
        "-hide_banner",
        "-re",
        "-async", "1",
        // "-thread_queue_size", "512",
        "-f", "image2",
        "-loop", "1",
        "-framerate", "25",
        "-i", "cover.png",
        "-f", "s16le",
        "-ac", "2",
        "-ar", "44100",
        "-i", "pipe:0",
    ];
    if env::var("USE_V4L2").is_ok() {
        args.append(&mut vec![
            "-c:v", "h264_v4l2m2m",
            "-num_output_buffers", "32",
            "-num_capture_buffers", "16",
        ]);
    } else {
        args.append(&mut vec![
            "-c:v", "libx264",
            "-crf", "23",
            "-preset", "ultrafast",
        ]);
    };
    args.append(&mut vec![
        "-c:a", "aac",
        "-b:a", "320k",
        "-f", "mpegts",
        &output,
    ]);
    let mut process = Command::new("ffmpeg")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // .stderr(Stdio::null())
        .spawn()
        .expect("Failed to execute main ffmpeg.");
    let mut process_stdout = process.stdout.take().expect("Failed to take stdout of ffmpeg(main)");
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let _ = tokio::io::copy(&mut process_stdout, &mut stdout).await;
    });

    let mut stdin = process.stdin.take().expect("Failed to take stdin of ffmpeg(main)");

    const PLAYLIST_SIZE: usize = 2;
    let mut playlist = VecDeque::with_capacity(PLAYLIST_SIZE);
    let tracks = manager.filter_tracks(&albums);

    // prefill playlist
    while playlist.len() != PLAYLIST_SIZE {
        let track = tracks.random();
        eprintln!("album_id = {}, disc_id = {}, track = {}", track.album_id, track.disc_id, track.track_id);

        if let Ok((audio, cover)) = generate_cover(provider.clone(), &track).await {
            if let Ok(cover) = save_cover(cover).await {
                playlist.push_back((track, audio, cover));
            }
        }
    }

    loop {
        let (track, audio, cover) = playlist.pop_front().unwrap();
        // apply actual cover
        tokio::fs::copy(cover, "cover.png").await?;
        // transcode audio to s16le
        let mut stdout = to_s16le(audio.reader).await?;
        // copy s16le to ffmpeg stdin & push new track to playlist concurrently
        let (copy, release_playlist) = tokio::join!(
            tokio::io::copy(&mut stdout, &mut stdin),
            (|| async {
                let track = tracks.random();
                eprintln!("album_id = {}, disc_id = {}, track = {}", track.album_id, track.disc_id, track.track_id);
                if let Ok((audio, cover)) = generate_cover(provider.clone(), &track).await {
                    if let Ok(cover) = save_cover(cover).await {
                        playlist.push_back((track, audio, cover));
                    }
                }
                playlist
            })()
        );
        playlist = release_playlist;

        provider.invalidate(track.album_id, track.disc_id as u8, track.track_id as u8);
        if matches!(copy, Err(_)) {
            break;
        }
    }

    Ok(())
}
