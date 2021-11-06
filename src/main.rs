mod repo;
mod utils;

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{ChildStdout, Command};
use anni_backend::{Backend, BackendReader, BackendReaderExt};
use anni_backend::backends::FileBackend;
use rand::Rng;
use tokio::io::AsyncWriteExt;
use crate::repo::RepoManager;

fn random_song(albums: &HashSet<String>, repo: &RepoManager) -> (String, usize) {
    loop {
        let mut rng = rand::thread_rng();
        let pos = rng.gen_range(0..albums.len());
        if let Some(catalog) = albums.iter().nth(pos) {
            let album = repo.load_album(catalog).unwrap();
            let tracks = album.discs()[0].tracks();
            let track_id = rng.gen_range(0..tracks.len());
            let ref track = tracks[track_id];
            let track_id = track_id + 1;
            use anni_repo::album::TrackType;
            match track.track_type() {
                TrackType::Normal | TrackType::Absolute => {
                    return (catalog.clone(), track_id);
                }
                _ => continue,
            }
        }
    }
}

async fn to_wav(mut reader: BackendReader) -> anyhow::Result<ChildStdout> {
    let mut cmd = Command::new("ffmpeg")
        .args([
            "-f", "flac",
            "-i", "pipe:0",
            "-f", "wav",
            // TODO: pcm_s16le 44100 stereo s16
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = cmd.stdin.take().expect("Failed to take stdin of ffmpeg(to_wav)");
    tokio::spawn(async move {
        let _ = tokio::io::copy(&mut reader, &mut stdin).await;
    });
    let mut stdout = cmd.stdout.take().expect("Failed to take stdout of ffmpeg(to_wav)");
    // TODO: calculate wav header size instead of fixed 196 offset
    utils::skip(&mut stdout, 196).await?;
    Ok(stdout)
}

async fn generate_cover(albums: Arc<HashSet<String>>, manager: Arc<RepoManager>, backend: Arc<FileBackend>) -> anyhow::Result<BackendReaderExt> {
    let (catalog, track_id) = random_song(&albums, &manager);
    eprintln!("catalog = {}, track = {}", catalog, track_id);
    let audio = backend.get_audio(&catalog, track_id as u8).await?;
    let mut cover = backend.get_cover(&catalog).await?;

    let album = manager.load_album(&catalog).unwrap();
    let track = &album.discs()[0].tracks()[track_id - 1];
    // TODO: i18n support for text
    let text = format!(r#"
序号：{}/{}
标题：{}
艺术家：{}
专辑：{}
"#, catalog, track_id, track.title(), track.artist(), album.title());
    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            // TODO: use relative path
            "-i", "/tmp/test/black.png",
            "-i", "pipe:0",
            "-filter_complex", &format!("[1:v]scale=-1:'min(1000,ih)'[ovrl],
    [0:v][ovrl]overlay=(main_w-overlay_w)/2:(main_h-overlay_h)/2,
    drawtext=
      fontfile=/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc:
      text='{}':
      x=8: y='main_h-text_h-8':
      fontcolor=white:
      borderw=2:
      fontsize=24", text),
            "-frames:v", "1",
            // "-f", "png_pipe", "-",
            "cover.png",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn().expect("Failed to generate image");
    let mut stdin = child.stdin.as_mut().expect("Failed to take stdin of ffmpeg(cover)");
    tokio::io::copy(&mut cover, &mut stdin).await.expect("Failed to copy cover from reader");
    Ok(audio)
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = RepoManager::new(env::var("ANNI_REPO")?);
    let mut backend = FileBackend::new(PathBuf::from(env::var("ARDIO_MUSIC")?), false);
    let albums = Arc::new(backend.albums().await?);

    let mut process = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-re",
            "-async", "1",
            "-f", "image2",
            "-loop", "1",
            "-framerate", "25",
            "-i", "cover.png",
            "-f", "wav",
            "-i", "pipe:0",
            "-c:v", "libx264",
            // "-c:v", "h264_omx",
            "-pixel_format", "yuv420p",
            "-crf", "16",
            "-preset", "ultrafast",
            "-maxrate", "1M",
            "-bufsize", "1M",
            "-c:a", "aac",
            "-b:a", "320k",
            "-f", "mpegts",
            &env::args().nth(1).unwrap_or(String::from("-")),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to execute main ffmpeg.");
    let mut process_stdout = process.stdout.take().expect("Failed to take stdout of ffmpeg(main)");
    tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let _ = tokio::io::copy(&mut process_stdout, &mut stdout).await;
    });

    let mut stdin = process.stdin.take().expect("Failed to take stdin of ffmpeg(main)");
    let manager = Arc::new(manager);
    let backend = Arc::new(backend);

    let wave_header = [
        0x52, 0x49, 0x46, 0x46, 0xFF, 0xFF, 0xFF, 0xFF, 0x57, 0x41, 0x56, 0x45, 0x66, 0x6D, 0x74, 0x20,
        0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x44, 0xAC, 0x00, 0x00, 0x10, 0xB1, 0x02, 0x00,
        0x04, 0x00, 0x10, 0x00, 0x64, 0x61, 0x74, 0x61, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    stdin.write_all(&wave_header).await.expect("Failed to write WAVE header");

    loop {
        if let Ok(audio) = generate_cover(albums.clone(), manager.clone(), backend.clone()).await {
            let mut stdout = to_wav(audio.reader).await?;
            tokio::io::copy(&mut stdout, &mut stdin).await?;
        }
    }
}
