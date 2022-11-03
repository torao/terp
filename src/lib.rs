//! Terp is an useful library that can generate various parsers based on the structure definition of a data sequence.
//! By defining a schema similar to a general grammer notation, users can parse a sequence of data without having to
//! implement a parser.
//!
//! <https://github.com/torao/terp>
//!
//! ## Examples
//!
//! The following example shows how to define a JSON-String schema compliant with
//! [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259.html) in a straightforward manner and parse it with terp, where
//! `A & B` means that `B` appears after `A`, `A | B` means that `A` or `B` appears, and `A * (X..=Y)` means `X` to `Y`
//! repetitions of `A`.
//!
//! ```rust
//! use terp::schema::{Schema, id, range};
//! use terp::parser::{Event, EventKind, Context};
//! use terp::schema::chars::{Location, ch, one_of_chars};
//!
//! // string = quotation-mark *char quotation-mark
//! // quotation-mark = '"'
//! // char = unescaped / escape ('"' / '\' / '/' / 'b' / 'f' / 'n' / 'r' / 't' / 'u' 4HEXDIG)
//! // escape = '/'
//! // unescaped = %x20-21 / %x23-5B / %x5D-10FFFF
//! let schema = Schema::new("JSON String")
//!   .define("String",    id("Quote") & (id("Char") * (0..)) & id("Quote"))
//!   .define("Quote",     ch('\"'))
//!   .define("Char",      id("Unescaped") | id("Escape") & (one_of_chars("\"\\/bfnrt") | (ch('u') & (id("Hex") * 4))))
//!   .define("Escape",    ch('\\'))
//!   .define("Unescaped", range('\x20'..='\x21') | range('\x23'..='\x5B') | range('\x5D'..='\u{10FFFF}'))
//!   .define("Hex",       range('0'..='9') | range('a'..='f') | range('A'..='F'));
//!
//! let mut events = Vec::new();
//! let mut parser = Context::new(&schema, "String", |e: &Event<&str, char>| events.push(e.clone()))
//!   .unwrap()
//!   .ignore_events_for(&["Char", "Escape", "Unescaped", "Hex"]);
//! parser.push_str("\"fo").unwrap();
//! parser.push_str("o").unwrap();
//! parser.push_str("\"").unwrap();
//! parser.finish().unwrap();
//!
//! let expected = vec![
//!   Event{ kind: EventKind::Begin("String"),                location: Location{ chars: 0, lines: 0, columns: 0} },
//!   Event{ kind: EventKind::Begin("Quote"),                 location: Location{ chars: 0, lines: 0, columns: 0} },
//!   Event{ kind: EventKind::Fragments(vec!['\"']),          location: Location{ chars: 0, lines: 0, columns: 0} },
//!   Event{ kind: EventKind::End("Quote"),                   location: Location{ chars: 1, lines: 0, columns: 1} },
//!   Event{ kind: EventKind::Fragments(vec!['f', 'o', 'o']), location: Location{ chars: 1, lines: 0, columns: 1} },
//!   Event{ kind: EventKind::Begin("Quote"),                 location: Location{ chars: 4, lines: 0, columns: 4} },
//!   Event{ kind: EventKind::Fragments(vec!['\"']),          location: Location{ chars: 4, lines: 0, columns: 4} },
//!   Event{ kind: EventKind::End("Quote"),                   location: Location{ chars: 5, lines: 0, columns: 5} },
//!   Event{ kind: EventKind::End("String"),                  location: Location{ chars: 5, lines: 0, columns: 5} },
//! ];
//! assert_eq!(expected, Event::normalize(&events));
//! ```
//!
#![allow(uncommon_codepoints)]

use schema::Symbol;

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

pub type Result<Σ, T> = std::result::Result<T, Error<Σ>>;

#[derive(thiserror::Error, Clone, Debug, PartialEq, Eq)]
pub enum Error<Σ: Symbol> {
  #[error("{location} {prefix}{expecteds:?} expected, but {prefix}{actual} appeared")]
  Unmatched {
    location: Σ::Location,
    prefix: String,
    expecteds: Vec<String>,
    expected_syntaxes: Vec<String>,
    actual: String,
  },
  #[error("{location} multiple syntax matches were found")]
  MultipleMatches { location: Σ::Location, prefix: String, expecteds: Vec<String>, actual: String },
  #[error("{0}")]
  UndefinedID(String),
  #[error("the previous error prevented progress")]
  Previous,
}
