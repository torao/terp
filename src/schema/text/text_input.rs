use std::io::SeekFrom;
use std::pin::Pin;
use std::{io, task::Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};

pub trait TextInput: AsyncReadExt + AsyncSeekExt + Send + Sync + Unpin {}

pub struct UTF8BytesInput {
  position: usize,
  buffer: Vec<u8>,
}

impl UTF8BytesInput {
  pub fn new(bytes: Vec<u8>) -> Self {
    Self { position: 0, buffer: bytes }
  }
}

impl TextInput for UTF8BytesInput {}

impl AsyncRead for UTF8BytesInput {
  fn poll_read(
    self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>, buf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<io::Result<()>> {
    let len = std::cmp::min(self.buffer.len() - self.position, buf.remaining());
    if len > 0 {
      buf.put_slice(&self.buffer[self.position..self.position + len]);
      self.get_mut().position += len;
    }
    Poll::Ready(Ok(()))
  }
}

impl AsyncSeek for UTF8BytesInput {
  fn start_seek(self: Pin<&mut Self>, position: SeekFrom) -> io::Result<()> {
    self.get_mut().position = match position {
      SeekFrom::Current(offset) => (self.position as i64 + offset) as usize,
      SeekFrom::Start(offset) => offset as usize,
      SeekFrom::End(offset) => (self.buffer.len() as i64 + offset) as usize,
    };
    Ok(())
  }

  fn poll_complete(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<io::Result<u64>> {
    std::task::Poll::Ready(Ok(self.position as u64))
  }
}

impl From<Vec<u8>> for UTF8BytesInput {
  fn from(bytes: Vec<u8>) -> Self {
    UTF8BytesInput::new(bytes)
  }
}

impl From<&str> for UTF8BytesInput {
  fn from(s: &str) -> Self {
    From::from(s.as_bytes().to_vec())
  }
}

impl From<String> for UTF8BytesInput {
  fn from(s: String) -> Self {
    From::from(s.as_bytes().to_vec())
  }
}
