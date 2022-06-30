use std::fmt::Display;

#[cfg(test)]
mod test;

#[derive(Default, Copy, Clone, Debug)]
pub struct Location(pub u64);

impl crate::schema::Location<u8> for Location {
  fn position(&self) -> u64 {
    self.0
  }
  fn increment_with(&mut self, _b: u8) {
    self.0 += 1;
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{}", self.0)
  }
}
