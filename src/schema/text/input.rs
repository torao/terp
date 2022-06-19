use crate::schema::{InputSource, Position};
use crate::{Error, Result};
use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_8};
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

pub struct BytesInputSource {
  is: CharInputSource<UTF8BytesInput>,
}

impl BytesInputSource {
  pub fn from_utf8_bytes(bytes: &[u8], replacement: bool) -> Self {
    let r = UTF8BytesInput::new(bytes.to_vec());
    Self { is: CharInputSource::<UTF8BytesInput>::new(r, UTF_8, replacement) }
  }

  pub fn from_string<S: AsRef<str>>(s: S) -> Self {
    Self::from_utf8_bytes(s.as_ref().as_bytes(), true)
  }
}

#[async_trait]
impl InputSource<char> for BytesInputSource {
  async fn read(&mut self) -> Result<Option<char>> {
    self.is.read().await
  }
  async fn unread(&mut self, length: usize) -> Result<()> {
    self.is.unread(length).await
  }
  async fn position(&mut self) -> Result<Position> {
    self.is.position().await
  }
  async fn seek(&mut self, position: Position) -> Result<()> {
    self.is.seek(position).await
  }
}

pub struct CharInputSource<IN: TextInput> {
  r: IN,
  encoding: &'static Encoding,
  decode_buf: Vec<u8>,
  unread_buf: Vec<char>,
  replacement: bool,
  max_mark: Option<Position>,
}

impl<IN: TextInput> CharInputSource<IN> {
  fn new<I: TextInput>(r: I, encoding: &'static Encoding, replacement: bool) -> CharInputSource<I> {
    let decode_buf = Vec::with_capacity(8);
    let unread_buf = Vec::new();
    let max_mark = None;
    CharInputSource { r, encoding, decode_buf, unread_buf, replacement, max_mark }
  }
}

#[async_trait]
impl<IN: TextInput> crate::schema::InputSource<char> for CharInputSource<IN> {
  async fn read(&mut self) -> Result<Option<char>> {
    // If there was a byte sequence with two or more characters that taken out in a *single decoding*, the continuation
    // of the preceding character is returned.
    if !self.unread_buf.is_empty() {
      return Ok(self.unread_buf.pop());
    }

    let mut buf = [0u8; 1];
    self.decode_buf.truncate(0);
    while self.decode_buf.len() < self.decode_buf.capacity() {
      // read a byte
      let len = self.r.read(&mut buf).await?;
      if len == 0 {
        if self.decode_buf.is_empty() {
          return Ok(None);
        } else {
          break;
        }
      }

      self.decode_buf.push(buf[0]);
      if let Some(s) = self.encoding.decode_without_bom_handling_and_without_replacement(&self.decode_buf) {
        // If the decoding was able to retrieved a valid character, the first character is returned. The remaining
        // characters, if any, are stored in the unread bufffer.
        let mut chars = s.chars().rev().collect::<Vec<_>>();
        if !chars.is_empty() {
          let ch = chars.pop();
          if !chars.is_empty() {
            debug_assert!(self.unread_buf.is_empty());
            self.unread_buf.append(&mut chars);
          }
          return Ok(ch);
        }

        // if decoding succeeds but no valid character is retrieved, the buffer is cleared and the next byte sequence
        // is read.
        self.decode_buf.truncate(0);
      }
    }

    // If the byte sequence isn't decodable, the first byte is reported as an illegal character.
    debug_assert!(!self.decode_buf.is_empty());
    if self.decode_buf.len() > 1 {
      self.r.seek(SeekFrom::Current(-(self.decode_buf.len() as i64 - 1))).await?;
    }
    if self.replacement {
      Ok(Some(char::REPLACEMENT_CHARACTER))
    } else {
      let encoding = self.encoding.name();
      let position = self.r.stream_position().await? - 1;
      let sequence = self.decode_buf.clone();
      Err(Error::CharacterDecoding { encoding, position, sequence })
    }
  }

  async fn unread(&mut self, length: usize) -> Result<()> {
    self.r.seek(SeekFrom::Current(-(length as i64))).await?;
    Ok(())
  }

  async fn position(&mut self) -> Result<Position> {
    let mark = self.r.stream_position().await?;
    if self.max_mark.map(|m| m < mark).unwrap_or(true) {
      self.max_mark = Some(mark);
    }
    Ok(mark)
  }

  async fn seek(&mut self, position: Position) -> Result<()> {
    // TODO: マークが発行されていない位置や、すでにより小さい位置の reset によって無効化されたマークを検出する処理が必要
    if self.max_mark.map(|m| m < position).unwrap_or(true) {
      Err(Error::OperationByIncorrectStreamMark(position))
    } else {
      self.r.seek(std::io::SeekFrom::Start(position)).await?;
      Ok(())
    }
  }
}
