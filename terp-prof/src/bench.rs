extern crate test;
use terp::parser::{Context, Event};
use terp::schema::json::{schema, ID};
use terp::schema::Schema;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const SAMPLE_WIKIPEDIA: &str = r#"
{
  "Image": {
      "Width":  800,
      "Height": 600,
      "Title":  "View from 15th Floor",
      "Thumbnail": {
          "Url":    "http://www.example.com/image/481989943",
          "Height": 125,
          "Width":  100
      },
      "Animated" : false,
      "IDs": [116, 943, 234, 38793]
    }
}"#;

#[bench]
fn rfc8259_schema_build(b: &mut test::Bencher) {
  b.iter(|| schema());
}

#[bench]
fn rfc8259_sample_wikipedia(b: &mut test::Bencher) {
  let schema = schema();
  b.iter(|| {
    let event_handler = |_: Event<ID, char>| ();
    let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
    parser.push_str(SAMPLE_WIKIPEDIA).unwrap();
    parser.finish().unwrap();
  });
}

#[bench]
fn various_json_files(b: &mut test::Bencher) {
  let naive_schema = naive_rfc8259_schema();
  for (name, path) in files("ok-").into_iter() {
    let content = fs::read_to_string(&path).unwrap();

    b.iter(|| {
      let event_handler = |_: Event<ID, char>| ();
      let mut parser = Context::new(&naive_schema, ID::JsonText, event_handler).unwrap();
      parser.push_str(&content).unwrap();
      parser.finish().unwrap();
    });
  }
}

fn naive_rfc8259_schema() -> Schema<ID, char> {
  use terp::schema::chars::*;
  use terp::schema::*;
  use terp::schema::json::ID::*;
  Schema::new("JSON")
    .define(JsonText, id(WS) & id(Value) & id(WS))
    .define(BeginArray, id(WS) & ch('['))
    .define(BeginObject, id(WS) & ch('{') & id(WS))
    .define(EndArray, id(WS) & ch(']') & id(WS))
    .define(EndObject, id(WS) & ch('}') & id(WS))
    .define(NameSeparator, id(WS) & ch(':') & id(WS))
    .define(ValueSeparator, id(WS) & ch(',') & id(WS))
    .define(WS, one_of(&[' ', '\t', '\x0A', '\x0D']) * (0..))
    .define(Value, id(False) | id(Null) | id(True) | id(Object) | id(Array) | id(Number) | id(String))
    .define(False, token("false"))
    .define(Null, token("null"))
    .define(True, token("true"))
    .define(
      Object,
      id(BeginObject)
        & ((id(Member) & ((id(ValueSeparator) & id(Member)) * (0..=usize::MAX))) * (0..=1))
        & id(EndObject),
    )
    .define(Member, id(String) & id(NameSeparator) & id(Value))
    .define(
      Array,
      id(BeginArray) & ((id(Value) & ((id(ValueSeparator) & id(Value)) * (0..=usize::MAX))) * (0..=1)) & id(EndArray),
    )
    .define(Number, (id(Minus) * (0..=1)) & id(Int) & (id(Frac) * (0..=1)) & (id(Exp) * (0..=1)))
    .define(DecimalPoint, ch('.'))
    .define(Digit1_9, range('1'..='9'))
    .define(E, one_of(&['e', 'E']))
    .define(Exp, id(E) & ((id(Minus) | id(Plus)) * (0..=1)) & (id(Digit) * (1..=usize::MAX)))
    .define(Frac, id(DecimalPoint) & (id(Digit) * (1..=usize::MAX)))
    .define(Int, id(Zero) | (id(Digit1_9) & (id(Digit) * (0..=usize::MAX))))
    .define(Minus, ch('-'))
    .define(Plus, ch('+'))
    .define(Zero, ch('0'))
    .define(String, id(QuotationMark) & (id(Char) * (0..=usize::MAX)) & id(QuotationMark))
    .define(Char, id(Unescaped) | id(Escape) & (one_of(&['\"', '\\', '/', 'b', 'f', 'n', 'r', 't']) | (id(HexDig) * 4)))
    .define(Escape, ch('\\'))
    .define(QuotationMark, ch('\"'))
    .define(Unescaped, range('\x20'..='\x21') | range('\x23'..='\x5B') | range('\x5D'..='\u{10FFFF}'))
    .define(Digit, range('0'..='9'))
    .define(HexDig, range('0'..='9') | range('a'..='f') | range('A'..='F'))
}

fn files(prefix: &str) -> HashMap<String, PathBuf> {
  fs::read_dir(Path::new("src").join("parser").join("test").join("data"))
    .unwrap()
    .into_iter()
    .map(|path| path.unwrap().path())
    .filter(|path| path.is_file())
    .map(|path| (path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap(), path))
    .filter(|(name, _)| name.starts_with(prefix))
    .map(|(name, path)| (name[prefix.len()..].to_string(), path))
    .collect::<HashMap<_, _>>()
}
