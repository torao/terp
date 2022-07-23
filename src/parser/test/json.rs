use crate::parser::test::Events;
use crate::parser::{Context, Event};
use crate::schema::json::{schema, ID};

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

#[test]
fn rfc8259_sample() {
  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap().ignore_events_for(&[
    ID::WS,
    ID::Unescaped,
    ID::Char,
    ID::QuotationMark,
    ID::NameSeparator,
    ID::ValueSeparator,
    ID::BeginObject,
    ID::EndObject,
    ID::BeginArray,
    ID::EndArray,
    ID::Digit1_9,
    ID::Digit,
    ID::Int,
    ID::Value,
  ]);
  parser.push_str(SAMPLE_WIKIPEDIA).unwrap();
  parser.finish().unwrap();
  for (i, e) in events.iter().enumerate() {
    eprintln!("[{}] {}: {:?}", i, e.location, e.kind)
  }
  Events::new()
    .begin(ID::JsonText)
    .fragments("\n")
    .begin(ID::Object)
    .fragments("{\n  ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Image\"")
    .end()
    .fragments(": ")
    .begin(ID::Object)
    .fragments("{\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Width\"")
    .end()
    .fragments(":  ")
    .begin(ID::Number)
    .fragments("800")
    .end()
    .end()
    .fragments(",\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Height\"")
    .end()
    .fragments(": ")
    .begin(ID::Number)
    .fragments("600")
    .end()
    .end()
    .fragments(",\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Title\"")
    .end()
    .fragments(":  ")
    .begin(ID::String)
    .fragments("\"View from 15th Floor\"")
    .end()
    .end()
    .fragments(",\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Thumbnail\"")
    .end()
    .fragments(": ")
    .begin(ID::Object)
    .fragments("{\n          ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Url\"")
    .end()
    .fragments(":    ")
    .begin(ID::String)
    .fragments("\"http://www.example.com/image/481989943\"")
    .end()
    .end()
    .fragments(",\n          ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Height\"")
    .end()
    .fragments(": ")
    .begin(ID::Number)
    .fragments("125")
    .end()
    .end()
    .fragments(",\n          ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Width\"")
    .end()
    .fragments(":  ")
    .begin(ID::Number)
    .fragments("100")
    .end()
    .end()
    .fragments("\n      }")
    .end()
    .end()
    .fragments(",\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"Animated\"")
    .end()
    .fragments(" : ")
    .begin(ID::False)
    .fragments("false")
    .end()
    .end()
    .fragments(",\n      ")
    .begin(ID::Member)
    .begin(ID::String)
    .fragments("\"IDs\"")
    .end()
    .fragments(": ")
    .begin(ID::Array)
    .fragments("[")
    .begin(ID::Number)
    .fragments("116")
    .end()
    .fragments(", ")
    .begin(ID::Number)
    .fragments("943")
    .end()
    .fragments(", ")
    .begin(ID::Number)
    .fragments("234")
    .end()
    .fragments(", ")
    .begin(ID::Number)
    .fragments("38793")
    .end()
    .fragments("]\n    ")
    .end()
    .end()
    .fragments("}\n")
    .end()
    .end()
    .fragments("}")
    .end()
    .end()
    .assert_eq(&events);
}

#[test]
fn rfc8259_string() {
  let sample = r#""foo""#;

  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::String, event_handler).unwrap();
  parser.push_str(sample).unwrap();
  parser.finish().unwrap();
  Events::new()
    .begin(ID::String)
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("f")
    .end()
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("o")
    .end()
    .end()
    .begin(ID::Char)
    .begin(ID::Unescaped)
    .fragments("o")
    .end()
    .end()
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .end()
    .assert_eq(&events);
}

#[test]
fn rfc8259_string_ignore_chars() {
  let sample = r#""foo""#;

  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser =
    Context::new(&schema, ID::String, event_handler).unwrap().ignore_events_for(&[ID::Unescaped, ID::Char]);
  parser.push_str(sample).unwrap();
  parser.finish().unwrap();
  Events::new()
    .begin(ID::String)
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .fragments("foo")
    .begin(ID::QuotationMark)
    .fragments("\"")
    .end()
    .end()
    .assert_eq(&events);
}
