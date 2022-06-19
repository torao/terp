use crate::schema::{InputSource, Range, Syntax};

mod input;
pub use input::*;

#[cfg(test)]
mod test;

pub const ASCII_DIGIT: Range<char> = Range::new('0'..='9');

pub fn ascii_digit<IS: InputSource<char>>() -> Syntax<char, IS> {
  Syntax::new(Box::new(ASCII_DIGIT))
}
