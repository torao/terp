use crate::Result;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::marker::Send;
use std::ops::{BitAnd, BitOr, Mul, RangeInclusive};

pub mod text;

#[cfg(test)]
mod test;

#[derive(Debug)]
pub struct Schema<E: Item> {
  syntax: HashMap<String, Syntax<E>>,
}

impl<E: Item> Schema<E> {
  pub fn new() -> Self {
    Schema { syntax: HashMap::new() }
  }

  #[must_use]
  pub fn define(mut self, name: &str, syntax: Syntax<E>) -> Self {
    let syntax = if let Primary::Seq(_) = &syntax.primary {
      syntax
    } else {
      Syntax { repetition: 1..=1, primary: Primary::Seq(vec![syntax]) }
    };
    self.syntax.insert(name.to_string(), syntax);
    self
  }

  pub fn names(&self) -> impl Iterator<Item = &str> {
    self.syntax.keys().map(|k| k.as_str())
  }

  pub fn get(&self, name: &str) -> Option<&Syntax<E>> {
    self.syntax.get(name)
  }
}

impl<E: Item> Display for Schema<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut names = self.syntax.keys().collect::<Vec<_>>();
    names.sort();
    for name in names {
      let syntax = self.syntax.get(name).unwrap();
      writeln!(f, "{} := {}", name, syntax)?;
    }
    Ok(())
  }
}

impl<E: Item> Default for Schema<E> {
  fn default() -> Self {
    Self::new()
  }
}

// macro_rules! schema {
//   ($name:ident : $($meta_identifier:ident := $($definition_list:expr)* , )+ ) => {
//     pub enum $name {
//       $($meta_identifier,)*
//     }
//     impl $name {
//       pub fn expr(&self) -> usize {
//         match self {
//           $($name::$meta_identifier => $($definition_list)*,)+
//         }
//       }
//     }
//   };
// }

// schema! {
//   MySchema:
//     One := 1 + 1,
//     Two := { usize::from_le_bytes([1, 0, 0, 0, 0, 0, 0, 0]) + 1 },
//     Three := todo!(),
// }

pub trait Item: Copy + PartialOrd + Debug + Send + Sync {
  type Location: Location<Self>;

  fn debug_symbol(value: Self) -> String {
    let values = [value];
    Self::debug_symbols(&values)
  }
  fn debug_symbols(values: &[Self]) -> String;
  fn debug_symbols_with_ellipsis(values: &[Self], ellipsis: bool) -> String {
    Self::debug_symbols(values) + if ellipsis { "..." } else { "[EOF]" }
  }
}

impl Item for char {
  type Location = CharLocation;

  fn debug_symbol(value: Self) -> String {
    format!("{:?}", value)
  }
  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| c.escape_debug().to_string()).collect::<String>()
  }
}

impl Item for u8 {
  type Location = ByteLocation;

  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| format!("{:02X}", c)).collect::<String>()
  }
}

pub trait Location<E: Item>: Default + Display + Copy + Clone {
  fn next_with(&mut self, item: E);
}

#[derive(Default, Copy, Clone)]
pub struct CharLocation {
  lines: u64,
  columns: u64,
}

impl Location<char> for CharLocation {
  fn next_with(&mut self, ch: char) {
    if ch == '\n' {
      self.lines += 1;
      self.columns = 1;
    } else {
      self.columns += 1;
    }
  }
}

impl Display for CharLocation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "({},{})", self.lines, self.columns)
  }
}

#[derive(Default, Copy, Clone)]
pub struct ByteLocation(u64);

impl Location<u8> for ByteLocation {
  fn next_with(&mut self, _b: u8) {
    self.0 += 1;
  }
}

impl Display for ByteLocation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{}", self.0)
  }
}

// ---------------------------------

#[derive(Clone, Copy)]
pub enum MatchResult {
  Match,
  MatchAndCanAcceptMore,
  Unmatch,
  UnmatchAndCanAcceptMore,
}

impl MatchResult {
  pub fn is_match(&self) -> bool {
    matches!(self, MatchResult::Match | MatchResult::MatchAndCanAcceptMore)
  }
}

pub trait Matcher<E: Item>: Display + Debug + Send + Sync {
  fn matches(&self, values: &[E]) -> Result<MatchResult>;
}

// ---------------------------------

#[derive(Debug, Clone)]
pub struct Range<E: Item>(std::ops::RangeInclusive<E>);

impl<E: Item> Range<E> {
  pub const fn new(r: std::ops::RangeInclusive<E>) -> Self {
    Range(r)
  }
  pub fn start(&self) -> E {
    *self.0.start()
  }
  pub fn end(&self) -> E {
    *self.0.end()
  }
}

impl<E: Item> Matcher<E> for Range<E> {
  fn matches(&self, values: &[E]) -> Result<MatchResult> {
    assert!(values.len() == 1);
    Ok(if self.0.contains(&values[0]) { MatchResult::Match } else { MatchResult::Unmatch })
  }
}

impl<E: Item> Display for Range<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}..={:?}", self.start(), self.end())
  }
}

// ---------------------------------

#[derive(Debug)]
pub struct Syntax<E: Item> {
  pub(crate) repetition: RangeInclusive<usize>,
  pub(crate) primary: Primary<E>,
}

impl<E: Item> Syntax<E> {
  pub fn new(matcher: Box<dyn Matcher<E>>) -> Self {
    Self { primary: Primary::Term(matcher), repetition: 1..=1 }
  }

  pub fn and(self, rhs: Syntax<E>) -> Self {
    let Syntax { primary: l_arm, repetition: l_range } = self;
    let Syntax { primary: r_arm, repetition: r_range } = rhs;
    match (l_arm, r_arm) {
      (Primary::Seq(mut lhs), Primary::Seq(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Seq(lhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (Primary::Seq(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { primary: rhs, repetition: r_range });
        let arm = Primary::Seq(lhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (lhs, Primary::Seq(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { primary: lhs, repetition: r_range });
        let arm = Primary::Seq(rhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { primary: lhs, repetition: l_range };
        let rhs = Syntax { primary: rhs, repetition: r_range };
        Syntax { primary: Primary::Seq(vec![lhs, rhs]), repetition: 1..=1 }
      }
    }
  }

  pub fn or(self, rhs: Syntax<E>) -> Self {
    let Syntax { primary: l_arm, repetition: l_range } = self;
    let Syntax { primary: r_arm, repetition: r_range } = rhs;
    match (l_arm, r_arm) {
      (Primary::Or(mut lhs), Primary::Or(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Or(lhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (Primary::Or(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { primary: rhs, repetition: r_range });
        let arm = Primary::Or(lhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (lhs, Primary::Or(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { primary: lhs, repetition: r_range });
        let arm = Primary::Or(rhs);
        Syntax { primary: arm, repetition: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { primary: lhs, repetition: l_range };
        let rhs = Syntax { primary: rhs, repetition: r_range };
        Syntax { primary: Primary::Or(vec![lhs, rhs]), repetition: 1..=1 }
      }
    }
  }

  pub fn reps(self, reps: usize) -> Self {
    self.repetitions(reps..=reps)
  }

  pub fn repetitions(self, reps: RangeInclusive<usize>) -> Self {
    let Syntax { primary: arm, repetition: range } = self;
    let min = *range.start() * reps.start();
    let max = *range.end() * reps.end();
    Syntax { primary: arm, repetition: RangeInclusive::new(min, max) }
  }
}

impl<E: Item> Display for Syntax<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let min = *self.repetition.start();
    let max = *self.repetition.end();
    let repetation = if min == 1 && max == 1 {
      String::from("")
    } else if min == max {
      format!(" {} {}", OP_REPEAT, min)
    } else {
      format!(" {} {}..={}", OP_REPEAT, min, max)
    };
    write!(f, "{}{}", self.primary, repetation)
  }
}

impl<E: Item> BitOr for Syntax<E> {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    self.or(rhs)
  }
}

impl<E: Item> BitAnd for Syntax<E> {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    self.and(rhs)
  }
}

impl<E: Item> Mul<usize> for Syntax<E> {
  type Output = Self;

  fn mul(self, rhs: usize) -> Self::Output {
    self * (rhs..=rhs)
  }
}

impl<E: Item> Mul<std::ops::Range<usize>> for Syntax<E> {
  type Output = Self;

  fn mul(self, rhs: std::ops::Range<usize>) -> Self::Output {
    self * (rhs.start..=rhs.end)
  }
}

impl<E: Item> Mul<RangeInclusive<usize>> for Syntax<E> {
  type Output = Self;

  fn mul(self, rhs: RangeInclusive<usize>) -> Self::Output {
    self.repetitions(rhs)
  }
}

#[derive(Debug)]
pub(crate) enum Primary<E: Item> {
  Term(Box<dyn Matcher<E>>),
  Seq(Vec<Syntax<E>>),
  Or(Vec<Syntax<E>>),
}

impl<E: Item> Display for Primary<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Primary::Term(parser) => Display::fmt(parser, f),
      Primary::Seq(terms) => {
        write!(f, "{}", terms[0])?;
        for term in terms.iter().skip(1) {
          write!(f, " {} {}", OP_CONCAT, term)?;
        }
        Ok(())
      }
      Primary::Or(terms) => {
        write!(f, "{}", terms[0])?;
        for term in terms.iter().skip(1) {
          write!(f, " {} {}", OP_CHOICE, term)?;
        }
        Ok(())
      }
    }
  }
}

pub(crate) const OP_CONCAT: &str = "&";
pub(crate) const OP_CHOICE: &str = "|";
pub(crate) const OP_REPEAT: &str = "*";
