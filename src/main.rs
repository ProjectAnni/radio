mod repo;
mod utils;

use std::collections::VecDeque;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::io::Write;
use tokio::process::{ChildStdout, Command};
use anni_backend::{Backend, BackendReader, BackendReaderExt};
use anni_backend::backends::FileBackend;
use tempfile::{NamedTempFile, TempPath};
use tokio::fs::File;
use crate::repo::{RepoManager, TrackRef};

async fn to_s16le(mut reader: BackendReader) -> anyhow::Result<ChildStdout> {
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

async fn generate_cover(backend: Arc<FileBackend>, TrackRef { catalog, track_id, album, track }: TrackRef<'_, '_>) -> anyhow::Result<(BackendReaderExt, ChildStdout)> {
    let audio = backend.get_audio(&catalog, track_id as u8).await?;
    let mut cover = backend.get_cover(&catalog).await?;

    // TODO: i18n support for text
    let text_temp_file = tempfile::NamedTempFile::new()?;
    let mut text_file = text_temp_file.as_file();
    write!(text_file, r#"
序号：{}/{}
标题：{}
艺术家：{}
专辑：{}
"#, catalog, track_id, track.title(), track.artist(), album.title())?;
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
        let _ = text_temp_file;
        let _ = child.wait().await;
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
    let mut backend = FileBackend::new(PathBuf::from(env::var("ANNI_RADIO_ROOT")?), false);
    let albums = backend.albums().await?;

    let mut process = Command::new("ffmpeg")
        .args([
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
            "-c:v", "libx264",
            // "-c:v", "h264_omx",
            "-crf", "23",
            "-preset", "ultrafast",
            "-c:a", "aac",
            "-b:a", "320k",
            "-f", "mpegts",
            &env::args().nth(1).unwrap_or(String::from("-")),
        ])
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
    let backend = Arc::new(backend);

    const PLAYLIST_SIZE: usize = 2;
    let mut playlist = VecDeque::with_capacity(PLAYLIST_SIZE);

    loop {
        if playlist.len() != PLAYLIST_SIZE {
            let track = manager.random_track(&albums);
            eprintln!("catalog = {}, track = {}", track.catalog, track.track_id);

            if let Ok((audio, cover)) = generate_cover(backend.clone(), track).await {
                if let Ok(cover) = save_cover(cover).await {
                    playlist.push_back((audio, cover));
                }
            }
        } else {
            // play mode
            let (audio, cover) = playlist.pop_front().unwrap();
            // TODO: do not ?
            tokio::fs::copy(cover, "cover.png").await?;
            let mut stdout = to_s16le(audio.reader).await?;
            if let Err(_) = tokio::io::copy(&mut stdout, &mut stdin).await {
                break;
            }
        }
    }

    Ok(())
}
