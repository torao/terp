use terp::parser::Context;
use terp::parser::Event;
use terp::schema::json::{schema, ID};

const SAMPLE: &str = r#"
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

fn main() {
  let event_handler = |_: &Event<ID, char>| ();
  let schema = self::schema();
  let mut parser = Context::new(&schema, ID::JsonText, event_handler).unwrap();
  for ch in SAMPLE.chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
}
