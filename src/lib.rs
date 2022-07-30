use schema::Item;

pub mod parser;
pub mod schema;

#[cfg(test)]
mod test;

#[cfg(debug_assertions)]
#[macro_export]
macro_rules! debug {
  () => { eprintln!("[{:20}:{:3}]", file!(), line!()) };
  ($fmt:expr) => {{ eprintln!("[{:20}:{:3}] {}", file!(), line!(), $fmt) }};
  ($fmt:expr, $($arg:tt)*) => {{ let s = format!($fmt, $($arg)*); eprintln!("[{:20}:{:3}] {}", file!(), line!(), s); }};
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
  #[error("{location} {prefix}{expecteds:?} expected, but {prefix}{actual} appeared")]
  Unmatched {
    location: E::Location,
    prefix: String,
    expecteds: Vec<String>,
    expected_syntaxes: Vec<String>,
    actual: String,
  },
  #[error("{location} multiple syntax matches were found")]
  MultipleMatches { location: E::Location, prefix: String, expecteds: Vec<String>, actual: String },
  #[error("{0}")]
  UndefinedID(String),
  #[error("the previous error prevented progress")]
  Previous,
}
