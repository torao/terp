pub mod parser;
pub mod schema;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error("{0} expected, but {1} appeared")]
  Unexpected(String, String),

  // InputSource
  #[error("failed to decode character in {encoding}: {sequence:?} @ {position}")]
  CharacterDecoding { encoding: &'static str, position: u64, sequence: Vec<u8> },
  #[error("the marked position {0} is incorrect")]
  OperationByIncorrectStreamMark(u64),

  #[error(transparent)]
  Io(#[from] std::io::Error),
}
