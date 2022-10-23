use crate::schema::{any_of_ranges_with_label, one_of, one_of_seqs, range_with_label, seq, single, Syntax};
use std::fmt::{Debug, Display};

#[cfg(test)]
mod test;

#[inline]
pub fn ch<ID>(ch: char) -> Syntax<ID, char> {
  single(ch)
}

#[inline]
pub fn token<ID>(token: &str) -> Syntax<ID, char> {
  seq(&token.chars().collect::<Vec<_>>())
}

#[inline]
pub fn one_of_tokens<ID>(tokens: &[&str]) -> Syntax<ID, char> {
  let tokens = tokens.iter().map(|i| i.chars().collect::<Vec<_>>()).collect::<Vec<_>>();
  one_of_seqs(&tokens)
}

#[derive(Default, Copy, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
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
pub fn one_of_chars<ID: Debug>(chars: &str) -> Syntax<ID, char> {
  one_of(&chars.chars().collect::<Vec<_>>())
}

#[inline]
pub fn ascii_digit<ID: Debug>() -> Syntax<ID, char> {
  range_with_label("ASCII_DIGIT", '0'..='9')
}

#[inline]
pub fn ascii_lower_alphabetic<ID>() -> Syntax<ID, char> {
  range_with_label("ASCII_LOWER", 'a'..='z')
}

#[inline]
pub fn ascii_upper_alphabetic<ID>() -> Syntax<ID, char> {
  range_with_label("ASCII_UPPER", 'A'..='Z')
}

#[inline]
pub fn ascii_alphabetic<ID>() -> Syntax<ID, char> {
  any_of_ranges_with_label("ASCII_ALPHA", vec!['A'..='Z', 'a'..='z'])
}
