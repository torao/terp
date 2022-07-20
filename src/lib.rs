use schema::Item;

pub mod parser;
pub mod schema;

#[cfg(test)]
mod test;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug {
  () => { eprintln!("[{}:{:3}]", file!(), line!()) };
  ($fmt:expr) => {{ eprintln!("[{}:{:3}] {}", file!(), line!(), $fmt) }};
  ($fmt:expr, $($arg:tt)*) => {{ let s = format!($fmt, $($arg)*); eprintln!("[{}:{:3}] {}", file!(), line!(), s); }};
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

#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
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
    // remove duplicate errors
    let mut i = 0;
    while i < errors.len() {
      if errors[0..i].iter().any(|e| e == &errors[i]) {
        errors.remove(i);
      } else {
        i += 1;
      }
    }

    if errors.len() == 1 {
      Err(errors.remove(0))
    } else {
      Err(Error::Multi(errors))
    }
  }
}
