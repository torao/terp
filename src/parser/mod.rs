pub trait Parser<'schema> {
  fn push(&mut self, ch: char) -> bool;
  fn eof(&mut self) -> bool;
}
