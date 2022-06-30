use crate::schema::{patterned_single_item, MatchResult, Syntax};
use crate::Result;
use std::fmt::{Debug, Display};

#[cfg(test)]
mod test;

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Location {
  pub chars: u64,
  pub lines: u64,
  pub columns: u64,
}

impl crate::schema::Location<char> for Location {
  fn position(&self) -> u64 {
    self.chars
  }
  fn increment_with(&mut self, ch: char) {
    self.chars += 1;
    if ch == '\n' {
      self.lines += 1;
      self.columns = 0;
    } else if ch == '\r' {
      self.columns = 0;
    } else {
      self.columns += 1;
    }
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "({},{})", self.lines + 1, self.columns + 1)
  }
}

#[inline]
pub fn ascii_digit<ID: Debug>() -> Syntax<ID, char> {
  patterned_single_item!(ASCII_DIGIT, '0'..='9')
}

#[inline]
pub fn ascii_lower_alphabetic<ID>() -> Syntax<ID, char> {
  patterned_single_item!(ASCII_LOWER, 'a'..='z')
}

#[inline]
pub fn ascii_upper_alphabetic<ID>() -> Syntax<ID, char> {
  patterned_single_item!(ASCII_UPPER, 'A'..='Z')
}

#[inline]
pub fn ascii_alphabetic<ID>() -> Syntax<ID, char> {
  patterned_single_item!(ASCII_ALPHA, 'A'..='Z' | 'a'..='z')
}
