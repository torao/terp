use crate::schema::chars::{ch, one_of_chars, token};
use crate::schema::{id, one_of, range, Schema};
use std::fmt::Display;

#[cfg(test)]
mod test;

#[derive(Hash, Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum ID {
  JsonText,
  BeginArray,
  BeginObject,
  EndArray,
  EndObject,
  NameSeparator,
  ValueSeparator,
  WS,
  Value,
  False,
  Null,
  True,
  Object,
  Member,
  Array,
  Number,
  DecimalPoint,
  Digit1_9,
  E,
  Exp,
  Frac,
  Int,
  Minus,
  Plus,
  Zero,
  String,
  Char,
  Escape,
  QuotationMark,
  Unescaped,
  Digit,
  HexDig,
}

impl Display for ID {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self)
  }
}

/// The JavaScript Object Notation (JSON) Data Interchange Format
/// <https://datatracker.ietf.org/doc/html/rfc8259>
///
pub fn schema() -> Schema<ID, char> {
  use ID::*;
  Schema::new("JSON")
    .define(JsonText, id(WS) & id(Value) & id(WS))
    .define(BeginArray, id(WS) & ch('[') & id(WS))
    .define(BeginObject, id(WS) & ch('{') & id(WS))
    .define(EndArray, id(WS) & ch(']') & id(WS))
    .define(EndObject, id(WS) & ch('}') & id(WS))
    .define(NameSeparator, id(WS) & ch(':') & id(WS))
    .define(ValueSeparator, id(WS) & ch(',') & id(WS))
    .define(WS, one_of_chars(" \t\x0A\x0D") * (0..))
    .define(Value, id(False) | id(Null) | id(True) | id(Object) | id(Array) | id(Number) | id(String))
    .define(False, token("false"))
    .define(Null, token("null"))
    .define(True, token("true"))
    .define(
      Object,
      id(BeginObject) & ((id(Member) & ((id(ValueSeparator) & id(Member)) * (0..))) * (0..=1)) & id(EndObject),
    )
    .define(Member, id(String) & id(NameSeparator) & id(Value))
    .define(Array, id(BeginArray) & ((id(Value) & ((id(ValueSeparator) & id(Value)) * (0..))) * (0..=1)) & id(EndArray))
    .define(Number, (id(Minus) * (0..=1)) & id(Int) & (id(Frac) * (0..=1)) & (id(Exp) * (0..=1)))
    .define(DecimalPoint, ch('.'))
    .define(Digit1_9, range('1'..='9'))
    .define(E, one_of(&['e', 'E']))
    .define(Exp, id(E) & ((id(Minus) | id(Plus)) * (0..=1)) & (id(Digit) * (1..)))
    .define(Frac, id(DecimalPoint) & (id(Digit) * (1..)))
    .define(Int, id(Zero) | (id(Digit1_9) & (id(Digit) * (0..))))
    .define(Minus, ch('-'))
    .define(Plus, ch('+'))
    .define(Zero, ch('0'))
    .define(String, id(QuotationMark) & (id(Char) * (0..)) & id(QuotationMark))
    .define(Char, id(Unescaped) | id(Escape) & (one_of_chars("\"\\/bfnrt") | (ch('u') & (id(HexDig) * 4))))
    .define(Escape, ch('\\'))
    .define(QuotationMark, ch('\"'))
    .define(Unescaped, range('\x20'..='\x21') | range('\x23'..='\x5B') | range('\x5D'..='\u{10FFFF}'))
    .define(Digit, range('0'..='9'))
    .define(HexDig, range('0'..='9') | range('a'..='f') | range('A'..='F'))
}
