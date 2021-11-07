use tokio::io::{AsyncRead, AsyncReadExt};

pub async fn skip<R>(reader: &mut R, len: u64) -> std::io::Result<u64>
    where R: AsyncRead + Unpin + Sized {
    tokio::io::copy(&mut reader.take(len), &mut tokio::io::sink()).await
}