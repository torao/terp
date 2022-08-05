use super::{schema, ID};
use crate::parser::{test::Events, Context, Event};

#[test]
fn char() {
  fn e<F: Fn(Events<ID>) -> Events<ID>>(f: F) -> Events<ID> {
    f(Events::new().begin(ID::Char)).end()
  }
  for (json_text, expected) in ('\u{20}'..='\u{21}')
    .chain('\u{23}'..='\u{5B}')
    .chain('\u{5D}'..='\u{7F}')
    .map(|c| (c.to_string(), e(|eb| eb.begin(ID::Unescaped).fragments(&c.to_string()).end())))
    .chain(
      vec!['\"', '\\', '/', 'b', 'f', 'n', 'r', 't']
        .iter()
        .map(|c| (format!("\\{}", c), e(|eb| eb.begin(ID::Escape).fragments("\\").end().fragments(&c.to_string())))),
    )
    .chain((0x00..=0xFF).flat_map(|i| vec![format!("{:04X}", i), format!("{:04x}", i)]).map(|hex| {
      (
        format!("\\u{}", hex),
        e(|eb| {
          eb.begin(ID::Escape)
            .fragments("\\")
            .end()
            .fragments("u")
            .begin(ID::HexDig)
            .fragments(&hex[0..1])
            .end()
            .begin(ID::HexDig)
            .fragments(&hex[1..2])
            .end()
            .begin(ID::HexDig)
            .fragments(&hex[2..3])
            .end()
            .begin(ID::HexDig)
            .fragments(&hex[3..4])
            .end()
        }),
      )
    }))
  {
    let events = parse(ID::Char, &json_text);
    expected.assert_eq(&events);
  }
}

#[test]
fn unescaped() {
  let json_text = " !#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[]^_`abcdefghijklmnopqrstuvwxyz{|}~\x7F";
  assert_eq!(
    ('\u{20}'..='\u{21}').chain('\u{23}'..='\u{5B}').chain('\u{5D}'..='\u{7F}').collect::<String>(),
    json_text
  );
  for i in 0..json_text.len() {
    let json_text = &json_text[i..i + 1];
    let events = parse(ID::Unescaped, json_text);
    Events::new().begin(ID::Unescaped).fragments(json_text).end().assert_eq(&events);
  }
}

#[test]
fn digit() {
  let json_text = "0123456789";
  for i in 0..json_text.len() {
    let json_text = &json_text[i..i + 1];
    let events = parse(ID::Digit, json_text);
    Events::new().begin(ID::Digit).fragments(json_text).end().assert_eq(&events);
  }
}

#[test]
fn hex_digit() {
  let json_text = "0123456789abcdefABCDEF";
  for i in 0..json_text.len() {
    let json_text = &json_text[i..i + 1];
    let events = parse(ID::HexDig, json_text);
    Events::new().begin(ID::HexDig).fragments(json_text).end().assert_eq(&events);
  }
}

fn parse(id: ID, json_text: &str) -> Vec<Event<ID, char>> {
  let mut events = Vec::with_capacity(256);
  let handler = |e: Event<ID, char>| events.push(e);
  let schema = schema();
  let mut parser = Context::new(&schema, id, handler).unwrap();
  parser.push_str(json_text).unwrap();
  parser.finish().unwrap();
  events
}
