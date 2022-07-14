//#![feature(test)]
use schema::Item;

pub mod parser;
pub mod schema;

#[cfg(test)]
mod test;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug {
  () => { eprintln!("[{}:{}]", file!(), line!()) };
  ($fmt:expr) => {{ eprintln!("[{}:{}] {}", file!(), line!(), $fmt) }};
  ($fmt:expr, $($arg:tt)*) => {{ let s = format!($fmt, $($arg)*); eprintln!("[{}:{}] {}", file!(), line!(), s); }};
}

#[cfg(not(debug_assertions))]
#[macro_export]
macro_rules! debug {
  ($first:expr) => {{ let _ = &$first; }};
  ($first:expr, $($arg:expr),*) => {{
    debug!($first);
    debug!($($arg),+);
  }};
}

pub type Result<E, T> = std::result::Result<T, Error<E>>;

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum Error<E: Item> {
  #[error("{location} {expected} expected, but {actual} appeared")]
  Unmatched { location: E::Location, expected: String, actual: String },
  #[error("multiple syntax matches were found")]
  MultipleMatches { location: E::Location, expecteds: Vec<String>, actual: String },
  #[error("{0:?}")]
  Multi(Vec<Error<E>>),
  #[error("{0}")]
  UndefinedID(String),
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
