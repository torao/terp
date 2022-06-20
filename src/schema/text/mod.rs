use crate::schema::{Range, Syntax};

#[cfg(test)]
mod test;

pub fn ascii_digit() -> Syntax<char> {
  Syntax::new(Box::new(Range::new('0'..='9')))
}
