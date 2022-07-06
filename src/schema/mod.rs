use crate::Result;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::marker::Send;
use std::ops::{BitAnd, BitOr, Mul, RangeInclusive};

pub mod bytes;
pub mod chars;

#[cfg(test)]
mod test;

pub struct Schema<ID, E: Item> {
  name: String,
  /// The top-level [`Syntax`] stored with the `ID` must be [`Primary::Seq`].
  defs: BTreeMap<ID, Syntax<ID, E>>,
}

impl<ID, E: 'static + Item> Schema<ID, E> {
  pub fn new(name: &str) -> Self {
    Self { name: name.to_string(), defs: BTreeMap::default() }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn ids(&self) -> impl Iterator<Item = &ID> {
    self.defs.keys()
  }
}

impl<ID: Ord, E: 'static + Item> Schema<ID, E> {
  pub fn define(mut self, id: ID, syntax: Syntax<ID, E>) -> Self {
    // the specified Syntax is wrapped in Primary::Seq and stored if it's not a Primary::Seq
    let syntax = syntax.conv_to_non_repeating_seq();
    self.defs.insert(id, syntax);
    self
  }

  pub fn get(&self, id: &ID) -> Option<&Syntax<ID, E>> {
    self.defs.get(id)
  }
}

impl<ID: Display + Debug, E: Item> Display for Schema<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}", self.name)?;
    for (id, syntax) in self.defs.iter() {
      writeln!(f, "  {:?} := {}", id, syntax)?;
    }
    Ok(())
  }
}

impl<ID: Debug, E: Item> Debug for Schema<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Schema").field("name", &self.name).field("definition_list", &self.defs).finish()
  }
}

pub trait Item: 'static + Copy + Clone + Send + Sync + PartialEq + Eq + Display + Debug {
  type Location: Location<Self>;

  fn debug_symbol(value: Self) -> String {
    let values = [value];
    Self::debug_symbols(&values)
  }
  fn debug_symbols(values: &[Self]) -> String;
}

impl Item for char {
  type Location = chars::Location;

  fn debug_symbol(value: Self) -> String {
    format!("{:?}", value)
  }
  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| c.escape_debug().to_string()).collect::<String>()
  }
}

impl Item for u8 {
  type Location = bytes::Location;

  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| format!("{:02X}", c)).collect::<String>()
  }
}

pub trait Location<E: Item>: Default + Copy + Display + Debug {
  fn position(&self) -> u64;

  fn increment_with(&mut self, item: E);

  fn increment_with_seq(&mut self, items: &[E]) {
    for item in items {
      self.increment_with(*item);
    }
  }
}

// ---------------------------------

pub struct Syntax<ID, E: Item> {
  pub location: Option<E::Location>,
  pub(crate) repetition: RangeInclusive<usize>,
  pub(crate) primary: Primary<ID, E>,
}

impl<ID, E: 'static + Item> Syntax<ID, E> {
  fn with_primary(primary: Primary<ID, E>) -> Self {
    Self { location: None, primary, repetition: 1..=1 }
  }

  pub fn from_id(id: ID) -> Self {
    Syntax::with_primary(Primary::Alias(id))
  }

  pub fn from_matcher(label: &str, f: fn(&[E]) -> Result<E, MatchResult>) -> Self {
    Syntax::with_primary(Primary::Term(Box::new(FnMatcher::new(label, f))))
  }
  pub fn repetition(&self) -> &RangeInclusive<usize> {
    &self.repetition
  }

  pub fn and(self, rhs: Syntax<ID, E>) -> Self {
    let Syntax { primary: l_arm, repetition: l_range, location: l_location } = self;
    let Syntax { primary: r_arm, repetition: r_range, location: r_location } = rhs;
    match (l_arm, r_arm) {
      (Primary::Seq(mut lhs), Primary::Seq(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Seq(lhs);
        Syntax { location: l_location, primary: arm, repetition: l_range }
      }
      (Primary::Seq(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { primary: rhs, repetition: r_range, location: r_location });
        let arm = Primary::Seq(lhs);
        Syntax { location: l_location, primary: arm, repetition: l_range }
      }
      (lhs, Primary::Seq(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { primary: lhs, repetition: r_range, location: r_location });
        let arm = Primary::Seq(rhs);
        Syntax { location: l_location, primary: arm, repetition: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { primary: lhs, repetition: l_range, location: l_location };
        let rhs = Syntax { primary: rhs, repetition: r_range, location: r_location };
        Syntax { location: l_location, primary: Primary::Seq(vec![lhs, rhs]), repetition: 1..=1 }
      }
    }
  }

  pub fn or(self, rhs: Syntax<ID, E>) -> Self {
    let Syntax { primary: l_arm, repetition: l_range, location: l_location } = self;
    let Syntax { primary: r_arm, repetition: r_range, location: r_location } = rhs;
    match (l_arm, r_arm) {
      (Primary::Or(mut lhs), Primary::Or(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Or(lhs);
        Syntax { primary: arm, repetition: l_range, location: l_location }
      }
      (Primary::Or(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { primary: rhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq());
        let arm = Primary::Or(lhs);
        Syntax { primary: arm, repetition: l_range, location: l_location }
      }
      (lhs, Primary::Or(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { primary: lhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq());
        let arm = Primary::Or(rhs);
        Syntax { primary: arm, repetition: l_range, location: l_location }
      }
      (lhs, rhs) => {
        let lhs = Syntax { primary: lhs, repetition: l_range, location: l_location }.conv_to_non_repeating_seq();
        let rhs = Syntax { primary: rhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq();
        Syntax { primary: Primary::Or(vec![lhs, rhs]), repetition: 1..=1, location: l_location }
      }
    }
  }

  pub fn reps(self, reps: RangeInclusive<usize>) -> Self {
    let Syntax { primary, repetition: range, location } = self;
    let min = *range.start() * reps.start();
    let max = *range.end() * reps.end();
    Syntax { primary, repetition: RangeInclusive::new(min, max), location }
  }

  fn conv_to_non_repeating_seq(self) -> Self {
    if matches!(self.primary, Primary::Seq(_)) && *self.repetition.start() == 1 && *self.repetition.end() == 1 {
      self
    } else {
      let location = self.location;
      Syntax { repetition: 1..=1, primary: Primary::Seq(vec![self]), location }
    }
  }
}

impl<E: 'static + Item> Syntax<String, E> {
  pub fn from_id_str<S: Into<String>>(id: S) -> Self {
    Syntax::with_primary(Primary::Alias(id.into()))
  }
}

impl<ID: Display + Debug, E: Item> Display for Syntax<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let min = *self.repetition.start();
    let max = *self.repetition.end();
    let show_reps = min != 1 || max != 1;
    let show_parenth = show_reps
      && match &self.primary {
        Primary::Term(_) => false,
        Primary::Alias(_) => false,
        Primary::Seq(seq) => seq.len() > 1,
        Primary::Or(seq) => seq.len() > 1,
      };
    if show_parenth {
      write!(f, "({})", self.primary)?;
    } else {
      Display::fmt(&self.primary, f)?;
    }
    if show_reps {
      if min == 0 && max == 1 {
        write!(f, "?")
      } else if min == max {
        write!(f, "{{{}}}", min)
      } else {
        write!(f, "{{{},{}}}", min, max)
      }
    } else {
      Ok(())
    }
  }
}

impl<ID: Debug, E: Item> Debug for Syntax<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Syntax").field("repetition", &self.repetition).field("primary", &self.primary).finish()
  }
}

impl<ID: Debug, E: 'static + Item> BitOr for Syntax<ID, E> {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    self.or(rhs)
  }
}

impl<ID: Debug, E: 'static + Item> BitAnd for Syntax<ID, E> {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    self.and(rhs)
  }
}

impl<ID: Debug, E: 'static + Item> Mul<usize> for Syntax<ID, E> {
  type Output = Self;

  fn mul(self, rhs: usize) -> Self::Output {
    self * (rhs..=rhs)
  }
}

impl<ID: Debug, E: 'static + Item> Mul<std::ops::Range<usize>> for Syntax<ID, E> {
  type Output = Self;

  fn mul(self, rhs: std::ops::Range<usize>) -> Self::Output {
    self * (rhs.start..=rhs.end)
  }
}

impl<ID: Debug, E: 'static + Item> Mul<RangeInclusive<usize>> for Syntax<ID, E> {
  type Output = Self;

  fn mul(self, rhs: RangeInclusive<usize>) -> Self::Output {
    self.reps(rhs)
  }
}

// ---------------------------------

pub(crate) const OP_CONCAT: &str = ",";
pub(crate) const OP_CHOICE: &str = " |";

pub(crate) enum Primary<ID, E: Item> {
  Term(Box<dyn Matcher<E>>),
  Alias(ID),
  Seq(Vec<Syntax<ID, E>>),
  Or(Vec<Syntax<ID, E>>),
}

impl<ID: Display + Debug, E: Item> Display for Primary<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Primary::Term(parser) => Display::fmt(parser, f),
      Primary::Alias(id) => Display::fmt(id, f),
      Primary::Seq(terms) => display(f, terms, OP_CONCAT),
      Primary::Or(terms) => display(f, terms, OP_CHOICE),
    }
  }
}

fn display<ID, E>(f: &mut std::fmt::Formatter<'_>, branches: &[Syntax<ID, E>], sep: &str) -> std::fmt::Result
where
  ID: Display + Debug,
  E: Item,
{
  write!(f, "{}", branches[0])?;
  for term in branches.iter().skip(1) {
    write!(f, "{} {}", sep, term)?;
  }
  Ok(())
}

impl<ID: Debug, E: Item> Debug for Primary<ID, E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Term(matcher) => f.debug_tuple("Term").field(matcher).finish(),
      Self::Alias(id) => f.debug_tuple("Alias").field(id).finish(),
      Self::Seq(seq) => f.debug_tuple("Seq").field(seq).finish(),
      Self::Or(branches) => f.debug_tuple("Or").field(branches).finish(),
    }
  }
}

// ---------------------------------

#[derive(Debug, Clone, Copy)]
pub enum MatchResult {
  Match(usize),
  Unmatch,
  MatchAndCanAcceptMore(usize),
  UnmatchAndCanAcceptMore,
}

impl MatchResult {
  pub fn is_match(&self) -> bool {
    matches!(self, MatchResult::Match(_) | MatchResult::MatchAndCanAcceptMore(_))
  }
}

pub trait Matcher<E: Item>: Display + Debug + Send + Sync {
  fn matches(&self, values: &[E]) -> Result<E, MatchResult>;
}

struct FnMatcher<E: Item>(String, fn(&[E]) -> Result<E, MatchResult>);

impl<E: Item> FnMatcher<E> {
  pub fn new(name: &str, f: fn(&[E]) -> Result<E, MatchResult>) -> FnMatcher<E> {
    FnMatcher(name.to_string(), f)
  }
}

impl<E: Item> Matcher<E> for FnMatcher<E> {
  fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
    (self.1)(values)
  }
}

impl<E: Item> Display for FnMatcher<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<E: Item> Debug for FnMatcher<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("FnMatcher").field(&self.0).finish()
  }
}

// ---------------------------------

#[macro_export]
macro_rules! patterned_single_item {
  ($name:ident, $p:pat) => {
    patterned_single_item!(char, $name, $p)
  };
  ($t:tt, $name:ident, $p:pat) => {{
    fn _single_pattern<E: $crate::schema::Item>(values: &[$t]) -> Result<E, MatchResult> {
      Ok(if values.is_empty() {
        MatchResult::UnmatchAndCanAcceptMore
      } else {
        if matches!(values[0], $p) {
          MatchResult::Match(1)
        } else {
          MatchResult::Unmatch
        }
      })
    }
    Syntax::from_matcher(stringify!($name), _single_pattern)
  }};
}

pub use patterned_single_item;

pub fn seq<ID, E: Item>(items: &[E]) -> Syntax<ID, E> {
  #[derive(Debug)]
  struct SeqMatcher<E: Item> {
    items: Vec<E>,
  }
  impl<E: Item> Matcher<E> for SeqMatcher<E> {
    fn matches(&self, values: &[E]) -> Result<E, MatchResult> {
      let min = std::cmp::min(self.items.len(), values.len());
      for (i, value) in values.iter().take(min).enumerate() {
        if *value != self.items[i] {
          return Ok(MatchResult::Unmatch);
        }
      }
      Ok(if min < self.items.len() { MatchResult::UnmatchAndCanAcceptMore } else { MatchResult::Match(min) })
    }
  }
  impl<E: Item> Display for SeqMatcher<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.write_str(&E::debug_symbols(&self.items))
    }
  }
  let items = items.to_vec();
  Syntax::with_primary(Primary::Term(Box::new(SeqMatcher { items })))
}
