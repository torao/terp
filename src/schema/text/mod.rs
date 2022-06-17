use crate::schema::{InputSource, Position, Range, Syntax};
use crate::{Error, Result};
use async_trait::async_trait;
use encoding_rs::{Encoding, UTF_8};
use std::io::SeekFrom;

mod text_input;
pub use text_input::*;

#[cfg(test)]
mod test;

pub const ASCII_DIGIT: Range<char> = Range::new('0'..='9');

pub fn ascii_digit<IS: InputSource<char>>() -> Syntax<char, IS> {
  Syntax::new(Box::new(ASCII_DIGIT))
}

pub struct BufferInputSource {
  is: CharInputSource<UTF8BytesInput>,
}

impl BufferInputSource {
  pub fn from_utf8_bytes(bytes: &[u8], replacement: bool) -> Self {
    let r = UTF8BytesInput::new(bytes.to_vec());
    Self { is: CharInputSource::<UTF8BytesInput>::new(r, UTF_8, replacement) }
  }

  pub fn from_string<S: AsRef<str>>(s: S) -> Self {
    Self::from_utf8_bytes(s.as_ref().as_bytes(), true)
  }
}

#[async_trait]
impl InputSource<char> for BufferInputSource {
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
