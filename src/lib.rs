use schema::Item;

pub mod parser;
pub mod schema;

pub type Result<E, T> = std::result::Result<T, Error<E>>;

#[derive(thiserror::Error, Debug)]
pub enum Error<E: Item> {
  #[error("{location} {expected} expected, but {actual} appeared")]
  Unmatched { location: E::Location, expected: String, actual: String },
  #[error("multiple syntax matches were found")]
  MultipleMatches { location: E::Location, expecteds: Vec<String>, actual: String },
  #[error("{0:?}")]
  Multi(Vec<Error<E>>),
  #[error("unable to continue due to a previous error or already finished")]
  UnableToContinue,
  #[error("{0}")]
  UndefinedID(String),

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

impl<E: Item> Error<E> {
  pub fn errors<T>(mut errors: Vec<Error<E>>) -> Result<E, T> {
    if errors.len() == 1 {
      Err(errors.remove(0))
    } else {
      Err(Error::Multi(errors))
    }
  }
}
