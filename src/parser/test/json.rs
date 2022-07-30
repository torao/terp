use crate::parser::test::Events;
use crate::parser::{Context, Event};
use crate::schema::json::{schema, ID};
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::fs;
use std::hash::Hash;
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

const IGNORE: &[ID] = &[
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
];

#[test]
fn rfc8259_empty_object() {
  let events = parse_json("{}");
  Events::new().begin(ID::JsonText).begin(ID::Object).fragments("{}").end().end().assert_eq(&events);

  let events = parse_json("\n\t{ \n\t }\n\t");
  Events::new()
    .begin(ID::JsonText)
    .fragments("\n\t")
    .begin(ID::Object)
    .fragments("{ \n\t }\n\t")
    .end()
    .end()
    .assert_eq(&events);
}

#[test]
fn rfc8259_sample() {
  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap().ignore_events_for(IGNORE);
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

#[test]
fn various_json_files() {
  let schema = schema();
  for (name, path) in files("ok-").into_iter() {
    eprintln!("----------- [{}] ------------", name);
    let content = fs::read_to_string(&path).unwrap();
    let event_handler = |_: Event<ID, char>| ();
    let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
    parser.push_str(&content).unwrap_or_else(|e| panic!("{:?}: for parsing {}", e, name));
    // for ch in content.chars() {
    //   parser.push(ch).unwrap_or_else(|e| panic!("{:?}: for parsing {}", e, name));
    // }
    parser.finish().unwrap_or_else(|e| panic!("{:?}: for parsing {}", e, name));
  }
}

fn parse_json(text: &str) -> Vec<Event<ID, char>>
where
  ID: Clone + Display + Debug + PartialEq + Eq + Hash,
{
  let mut events = Vec::new();
  let event_handler = |e: Event<ID, char>| {
    println!("> {:?}", e);
    events.push(e);
  };
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap().ignore_events_for(IGNORE);
  parser.push_str(text).unwrap();
  parser.finish().unwrap();
  events
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
