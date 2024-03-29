use crate::Result;
use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::marker::Send;
use std::ops::{BitAnd, BitOr, Mul, RangeFrom, RangeInclusive, RangeTo, RangeToInclusive};

pub mod bytes;
pub mod chars;
pub mod json;

mod matcher;
pub use matcher::*;

#[cfg(test)]
mod test;

pub struct Schema<ID, Σ: Symbol> {
  name: String,
  syntax_id_seq: usize,
  /// The top-level [`Syntax`] stored with the `ID` must be [`Primary::Seq`].
  defs: BTreeMap<ID, Syntax<ID, Σ>>,
}

impl<ID, Σ: 'static + Symbol> Schema<ID, Σ> {
  pub fn new(name: &str) -> Self {
    Self { name: name.to_string(), syntax_id_seq: 1, defs: BTreeMap::default() }
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub fn ids(&self) -> impl Iterator<Item = &ID> {
    self.defs.keys()
  }
}

impl<ID: Ord, Σ: 'static + Symbol> Schema<ID, Σ> {
  pub fn define(mut self, id: ID, syntax: Syntax<ID, Σ>) -> Self {
    // the specified Syntax is wrapped in Primary::Seq if it's not a Primary::Seq
    let mut syntax = syntax.conv_to_non_repeating_seq();
    self.init_syntax_ids(&mut syntax);
    self.defs.insert(id, syntax);
    self
  }

  pub fn get(&self, id: &ID) -> Option<&Syntax<ID, Σ>> {
    self.defs.get(id)
  }

  fn init_syntax_ids(&mut self, syntax: &mut Syntax<ID, Σ>) {
    syntax.id = self.syntax_id_seq;
    self.syntax_id_seq += 1;
    match &mut syntax.primary {
      Primary::Term(..) => (),
      Primary::Alias(_) => (),
      Primary::Seq(branches) => {
        for branch in branches {
          self.init_syntax_ids(branch);
        }
      }
      Primary::Or(branches) => {
        for branch in branches {
          self.init_syntax_ids(branch);
        }
      }
    }
  }
}

impl<ID: Display + Debug, Σ: Symbol> Display for Schema<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(f, "{}", self.name)?;
    for (id, syntax) in self.defs.iter() {
      writeln!(f, "  {:?} := {}", id, syntax)?;
    }
    Ok(())
  }
}

impl<ID: Debug, Σ: Symbol> Debug for Schema<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Schema").field("name", &self.name).field("definition_list", &self.defs).finish()
  }
}

/// `Symbol` represents the type of 'terminal string' targeted by the schema or parser.
///
pub trait Symbol: 'static + Copy + Clone + Send + Sync + Hash + PartialEq + Eq + Display + Debug {
  type Location: Location<Self>;

  /// The number of items to be restored from the buffer for error messages. A maximum of 3 units and two three-point
  /// leaders will be given. e.g., in case 8 for `char`, `...3456789012345678...34567890`
  ///
  const SAMPLING_UNIT_AT_ERROR: usize;

  fn debug_symbol(value: Self) -> String {
    let values = [value];
    Self::debug_symbols(&values)
  }
  fn debug_symbols(values: &[Self]) -> String;
}

impl Symbol for char {
  type Location = chars::Location;
  const SAMPLING_UNIT_AT_ERROR: usize = 12;

  fn debug_symbol(value: Self) -> String {
    format!("{:?}", value)
  }
  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| c.escape_debug().to_string()).collect::<String>()
  }
}

impl Symbol for u8 {
  type Location = bytes::Location;
  const SAMPLING_UNIT_AT_ERROR: usize = 8;

  fn debug_symbols(values: &[Self]) -> String {
    values.iter().map(|c| format!("{:02X}", c)).collect::<String>()
  }
}

pub trait Location<Σ: Symbol>: Default + Copy + Display + Debug + Ord + PartialEq + Send + Sync {
  fn position(&self) -> u64;

  fn increment_with(&mut self, item: Σ);

  fn increment_with_seq(&mut self, items: &[Σ]) {
    for item in items {
      self.increment_with(*item);
    }
  }
}

// ---------------------------------

pub struct Syntax<ID, Σ: Symbol> {
  pub id: usize,
  pub location: Option<Σ::Location>,
  pub(crate) repetition: RangeInclusive<usize>,
  pub(crate) primary: Primary<ID, Σ>,
}

impl<ID, Σ: 'static + Symbol> Syntax<ID, Σ> {
  fn with_primary(primary: Primary<ID, Σ>) -> Self {
    Self { id: 0, location: None, primary, repetition: 1..=1 }
  }

  pub fn from_id(id: ID) -> Self {
    Syntax::with_primary(Primary::Alias(id))
  }

  pub fn from_fn<FN: Fn(&[Σ]) -> Result<Σ, MatchResult> + Send + Sync + 'static>(label: &str, f: FN) -> Self {
    Syntax::with_primary(Primary::Term(label.to_string(), Box::new(f)))
  }

  pub fn repetition(&self) -> &RangeInclusive<usize> {
    &self.repetition
  }

  pub fn and(self, rhs: Syntax<ID, Σ>) -> Self {
    let Syntax { id: l_id, primary: l_arm, repetition: l_range, location: l_location } = self;
    let Syntax { id: r_id, primary: r_arm, repetition: r_range, location: r_location } = rhs;
    debug_assert!(l_id == 0 && r_id == 0);
    match (l_arm, r_arm) {
      (Primary::Seq(mut lhs), Primary::Seq(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Seq(lhs);
        Syntax { id: 0, location: l_location, primary: arm, repetition: l_range }
      }
      (Primary::Seq(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { id: 0, primary: rhs, repetition: r_range, location: r_location });
        let arm = Primary::Seq(lhs);
        Syntax { id: 0, location: l_location, primary: arm, repetition: l_range }
      }
      (lhs, Primary::Seq(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { id: 0, primary: lhs, repetition: r_range, location: r_location });
        let arm = Primary::Seq(rhs);
        Syntax { id: 0, location: l_location, primary: arm, repetition: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { id: 0, primary: lhs, repetition: l_range, location: l_location };
        let rhs = Syntax { id: 0, primary: rhs, repetition: r_range, location: r_location };
        Syntax { id: 0, location: l_location, primary: Primary::Seq(vec![lhs, rhs]), repetition: 1..=1 }
      }
    }
  }

  pub fn or(self, rhs: Syntax<ID, Σ>) -> Self {
    let Syntax { id: l_id, primary: l_arm, repetition: l_range, location: l_location } = self;
    let Syntax { id: r_id, primary: r_arm, repetition: r_range, location: r_location } = rhs;
    debug_assert!(l_id == 0 && r_id == 0);
    match (l_arm, r_arm) {
      (Primary::Or(mut lhs), Primary::Or(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = Primary::Or(lhs);
        Syntax { id: 0, primary: arm, repetition: l_range, location: l_location }
      }
      (Primary::Or(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { id: 0, primary: rhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq());
        let arm = Primary::Or(lhs);
        Syntax { id: 0, primary: arm, repetition: l_range, location: l_location }
      }
      (lhs, Primary::Or(mut rhs)) if l_range == r_range => {
        rhs.insert(
          0,
          Syntax { id: 0, primary: lhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq(),
        );
        let arm = Primary::Or(rhs);
        Syntax { id: 0, primary: arm, repetition: l_range, location: l_location }
      }
      (lhs, rhs) => {
        let lhs = Syntax { id: 0, primary: lhs, repetition: l_range, location: l_location }.conv_to_non_repeating_seq();
        let rhs = Syntax { id: 0, primary: rhs, repetition: r_range, location: r_location }.conv_to_non_repeating_seq();
        Syntax { id: 0, primary: Primary::Or(vec![lhs, rhs]), repetition: 1..=1, location: l_location }
      }
    }
  }

  pub fn reps(self, reps: RangeInclusive<usize>) -> Self {
    let Syntax { id, primary, repetition: range, location } = self;
    debug_assert_eq!(0, id);
    let min = *range.start() * reps.start();
    let max = *range.end() * reps.end();
    Syntax { id: 0, primary, repetition: RangeInclusive::new(min, max), location }
  }

  fn conv_to_non_repeating_seq(self) -> Self {
    if matches!(self.primary, Primary::Seq(_)) && *self.repetition.start() == 1 && *self.repetition.end() == 1 {
      self
    } else {
      let location = self.location;
      Syntax { id: 0, repetition: 1..=1, primary: Primary::Seq(vec![self]), location }
    }
  }
}

impl<Σ: 'static + Symbol> Syntax<String, Σ> {
  pub fn from_id_str<S: Into<String>>(id: S) -> Self {
    Syntax::with_primary(Primary::Alias(id.into()))
  }
}

impl<ID: Display + Debug, Σ: Symbol> Display for Syntax<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let min = *self.repetition.start();
    let max = *self.repetition.end();
    let show_reps = min != 1 || max != 1;
    let show_parenth = show_reps
      && match &self.primary {
        Primary::Term(..) => false,
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
      } else if min == 0 && max == usize::MAX {
        write!(f, "*")
      } else if min == 1 && max == usize::MAX {
        write!(f, "+")
      } else if min == max {
        write!(f, "{{{}}}", min)
      } else if max == usize::MAX {
        write!(f, "{{{},}}", min)
      } else if min == 0 {
        write!(f, "{{,{}}}", max)
      } else {
        write!(f, "{{{},{}}}", min, max)
      }
    } else {
      Ok(())
    }
  }
}

impl<ID: Debug, Σ: Symbol> Debug for Syntax<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Syntax").field("repetition", &self.repetition).field("primary", &self.primary).finish()
  }
}

impl<ID: Debug, Σ: 'static + Symbol> BitOr for Syntax<ID, Σ> {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    self.or(rhs)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> BitAnd for Syntax<ID, Σ> {
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    self.and(rhs)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<usize> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: usize) -> Self::Output {
    self * (rhs..=rhs)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<std::ops::Range<usize>> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: std::ops::Range<usize>) -> Self::Output {
    self * (rhs.start..=rhs.end - 1)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<RangeInclusive<usize>> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: RangeInclusive<usize>) -> Self::Output {
    self.reps(rhs)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<RangeFrom<usize>> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: RangeFrom<usize>) -> Self::Output {
    self.reps(rhs.start..=usize::MAX)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<RangeTo<usize>> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: RangeTo<usize>) -> Self::Output {
    self * (0..=rhs.end - 1)
  }
}

impl<ID: Debug, Σ: 'static + Symbol> Mul<RangeToInclusive<usize>> for Syntax<ID, Σ> {
  type Output = Self;

  fn mul(self, rhs: RangeToInclusive<usize>) -> Self::Output {
    self.reps(0..=rhs.end)
  }
}

// ---------------------------------

pub(crate) const OP_CONCAT: &str = ",";
pub(crate) const OP_CHOICE: &str = " |";

pub type Matcher<Σ> = dyn Fn(&[Σ]) -> Result<Σ, MatchResult> + Send + Sync;

pub(crate) enum Primary<ID, Σ: Symbol> {
  Term(String, Box<Matcher<Σ>>),
  /// This corresponds to the so-called non-terminal character.
  Alias(ID),
  Seq(Vec<Syntax<ID, Σ>>),
  Or(Vec<Syntax<ID, Σ>>),
}

impl<ID: Display + Debug, Σ: Symbol> Display for Primary<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Primary::Term(name, ..) => Display::fmt(name, f),
      Primary::Alias(id) => Display::fmt(id, f),
      Primary::Seq(terms) => display(f, terms, OP_CONCAT),
      Primary::Or(terms) => display(f, terms, OP_CHOICE),
    }
  }
}

fn display<ID, Σ>(f: &mut std::fmt::Formatter<'_>, branches: &[Syntax<ID, Σ>], sep: &str) -> std::fmt::Result
where
  ID: Display + Debug,
  Σ: Symbol,
{
  write!(f, "{}", branches[0])?;
  for term in branches.iter().skip(1) {
    write!(f, "{} {}", sep, term)?;
  }
  Ok(())
}

impl<ID: Debug, Σ: Symbol> Debug for Primary<ID, Σ> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Term(name, ..) => f.debug_tuple("Term").field(name).finish(),
      Self::Alias(id) => f.debug_tuple("Alias").field(id).finish(),
      Self::Seq(seq) => f.debug_tuple("Seq").field(seq).finish(),
      Self::Or(branches) => f.debug_tuple("Or").field(branches).finish(),
    }
  }
}

// ---------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
