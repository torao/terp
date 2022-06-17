use crate::{Error, Result};
use async_recursion::async_recursion;
use async_trait::async_trait;
use std::fmt::{Debug, Display};
use std::marker::PhantomData;
use std::marker::Send;
use std::ops::{BitAnd, BitOr, Mul, RangeInclusive};

pub mod text;

pub struct Schema {}

pub trait Item: Copy + PartialOrd + Debug + Send + Sync {}

impl Item for char {}

impl Item for u8 {}

#[async_trait]
pub trait Parser<E, INPUT>: Display + Send + Sync
where
  E: Item,
  INPUT: InputSource<E>,
{
  async fn parse(&self, context: &mut Context<E, INPUT>) -> Result<bool>;
}

pub struct Context<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  pub is: INPUT,
  _e: PhantomData<E>,
}

pub type Position = u64;

#[async_trait]
pub trait InputSource<E: Item>: Send + Sync {
  async fn read(&mut self) -> Result<Option<E>>;
  async fn unread(&mut self, length: usize) -> Result<()>;
  async fn position(&mut self) -> Result<Position>;
  async fn seek(&mut self, position: Position) -> Result<()>;
}

// ---------------------------------

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

#[derive(Clone)]
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

#[async_trait]
impl<E, INPUT> Parser<E, INPUT> for Range<E>
where
  E: Item,
  INPUT: InputSource<E>,
{
  async fn parse(&self, c: &mut Context<E, INPUT>) -> Result<bool> {
    if let Some(ch) = c.is.read().await? {
      if self.0.contains(&ch) {
        return Ok(true);
      }
    }
    c.is.unread(1).await?;
    Ok(false)
  }
}

impl<E: Item> Display for Range<E> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}..={:?}", self.start(), self.end())
  }
}

pub struct Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  arm: SyntaxArm<E, INPUT>,
  range: RangeInclusive<usize>,
}

impl<E, INPUT> Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  pub fn new(parser: Box<dyn Parser<E, INPUT>>) -> Self {
    Self { arm: SyntaxArm::Term(parser), range: 1..=1 }
  }

  pub async fn parse(&self, is: INPUT) -> Result<()> {
    let mut context = Context { is, _e: PhantomData::default() };
    if self.parse_with(&mut context).await? {
      if let Some(ch) = context.is.read().await? {
        context.is.unread(1).await?;
        Err(Error::Unexpected(String::from("EOF"), format!("{:?}", ch)))
      } else {
        Ok(())
      }
    } else {
      Err()
    }
  }

  #[async_recursion]
  async fn parse_with(&self, context: &mut Context<E, INPUT>) -> Result<bool> {
    match &self.arm {
      SyntaxArm::Term(parser) => parser.parse(context).await,
      SyntaxArm::Seq(terms) => {
        for term in terms.iter() {
          if !term.parse_with(context).await? {
            return Ok(false);
          }
        }
        Ok(true)
      }
      SyntaxArm::Or(terms) => {
        let position = context.is.position().await?;
        let mut longest = position;
        for i in 0..terms.len() {
          if i != 0 {
            context.is.seek(position).await?;
          }
          if terms[i].parse_with(context).await? {
            let new_position = context.is.position().await?;
            assert!(new_position > position);
            if new_position > longest {
              if i + 1 == terms.len() {
                return Ok(true);
              }
              longest = new_position;
            }
          }
        }
        Ok(if longest > position {
          context.is.seek(longest).await?;
          true
        } else {
          false
        })
      }
    }
  }

  pub fn and(self, rhs: Syntax<E, INPUT>) -> Self {
    let Syntax { arm: l_arm, range: l_range } = self;
    let Syntax { arm: r_arm, range: r_range } = rhs;
    match (l_arm, r_arm) {
      (SyntaxArm::Seq(mut lhs), SyntaxArm::Seq(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = SyntaxArm::Seq(lhs);
        Syntax { arm, range: l_range }
      }
      (SyntaxArm::Seq(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { arm: rhs, range: r_range });
        let arm = SyntaxArm::Seq(lhs);
        Syntax { arm, range: l_range }
      }
      (lhs, SyntaxArm::Seq(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { arm: lhs, range: r_range });
        let arm = SyntaxArm::Seq(rhs);
        Syntax { arm, range: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { arm: lhs, range: l_range };
        let rhs = Syntax { arm: rhs, range: r_range };
        Syntax { arm: SyntaxArm::Seq(vec![lhs, rhs]), range: 1..=1 }
      }
    }
  }

  pub fn or(self, rhs: Syntax<E, INPUT>) -> Self {
    let Syntax { arm: l_arm, range: l_range } = self;
    let Syntax { arm: r_arm, range: r_range } = rhs;
    match (l_arm, r_arm) {
      (SyntaxArm::Or(mut lhs), SyntaxArm::Or(mut rhs)) if l_range == r_range => {
        lhs.append(&mut rhs);
        let arm = SyntaxArm::Or(lhs);
        Syntax { arm, range: l_range }
      }
      (SyntaxArm::Or(mut lhs), rhs) if l_range == r_range => {
        lhs.push(Syntax { arm: rhs, range: r_range });
        let arm = SyntaxArm::Or(lhs);
        Syntax { arm, range: l_range }
      }
      (lhs, SyntaxArm::Or(mut rhs)) if l_range == r_range => {
        rhs.insert(0, Syntax { arm: lhs, range: r_range });
        let arm = SyntaxArm::Or(rhs);
        Syntax { arm, range: l_range }
      }
      (lhs, rhs) => {
        let lhs = Syntax { arm: lhs, range: l_range };
        let rhs = Syntax { arm: rhs, range: r_range };
        Syntax { arm: SyntaxArm::Or(vec![lhs, rhs]), range: 1..=1 }
      }
    }
  }

  pub fn reps(self, reps: usize) -> Self {
    self.repetitions(reps..=reps)
  }

  pub fn repetitions(self, reps: RangeInclusive<usize>) -> Self {
    let Syntax { arm, range } = self;
    let min = *range.start() * reps.start();
    let max = *range.end() * reps.end();
    Syntax { arm, range: RangeInclusive::new(min, max) }
  }
}

impl<E, INPUT> Display for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let min = *self.range.start();
    let max = *self.range.end();
    let repetation = if min == 1 && max == 1 {
      String::from("")
    } else if min == max {
      format!(" * {}", min)
    } else {
      format!(" * {}..={}", min, max)
    };
    write!(f, "{}{}", self.arm, repetation)
  }
}

impl<E, INPUT> BitOr for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self::Output {
    self.or(rhs)
  }
}

impl<E, INPUT> BitAnd for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  type Output = Self;

  fn bitand(self, rhs: Self) -> Self::Output {
    self.and(rhs)
  }
}

impl<E, INPUT> Mul<usize> for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  type Output = Self;

  fn mul(self, rhs: usize) -> Self::Output {
    self * (rhs..=rhs)
  }
}

impl<E, INPUT> Mul<std::ops::Range<usize>> for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  type Output = Self;

  fn mul(self, rhs: std::ops::Range<usize>) -> Self::Output {
    self * (rhs.start..=rhs.end)
  }
}

impl<E, INPUT> Mul<RangeInclusive<usize>> for Syntax<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  type Output = Self;

  fn mul(self, rhs: RangeInclusive<usize>) -> Self::Output {
    self.repetitions(rhs)
  }
}

enum SyntaxArm<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  Term(Box<dyn Parser<E, INPUT>>),
  Seq(Vec<Syntax<E, INPUT>>),
  Or(Vec<Syntax<E, INPUT>>),
}

impl<E, INPUT> Display for SyntaxArm<E, INPUT>
where
  E: Item,
  INPUT: InputSource<E>,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SyntaxArm::Term(parser) => parser.fmt(f),
      SyntaxArm::Seq(terms) => {
        write!(f, "{}", terms[0])?;
        for term in terms.iter().skip(1) {
          write!(f, " & {}", term)?;
        }
        Ok(())
      }
      SyntaxArm::Or(terms) => {
        write!(f, "{}", terms[0])?;
        for term in terms.iter().skip(1) {
          write!(f, " | {}", term)?;
        }
        Ok(())
      }
    }
  }
}
