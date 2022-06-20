pub mod parser;
pub mod schema;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error("Unmatched")]
  Unmatched(),
  #[error("{0} expected, but {1} appeared")]
  Unexpected(String, String),
  #[error("")]
  MultipleMatches(),
  #[error("{0:?}")]
  Multi(Vec<Error>),
  #[error("Cannot match anymore")]
  CantMatchAnymore,

  // InputSource
  #[error("failed to decode character in {encoding}: {sequence:?} @ {position}")]
  CharacterDecoding { encoding: &'static str, position: u64, sequence: Vec<u8> },
  #[error("the marked position {0} is incorrect")]
  OperationByIncorrectStreamMark(u64),
  #[error("invalid seek to a negative or overflowing position")]
  InvalidSeek(i64),

  #[error(transparent)]
  Io(#[from] std::io::Error),
}

impl Error {
  pub fn errors<T>(mut errors: Vec<Error>) -> Result<T> {
    if errors.len() == 1 {
      Err(errors.remove(0))
    } else {
      Err(Error::Multi(errors))
    }
  }
}
