use crate::parser::{Context, Event};
use crate::schema::chars::{ascii_alphabetic, ascii_digit, ch};
use crate::schema::Schema;

#[test]
fn or() {
  let a = ((ascii_digit() | ascii_alphabetic()) * (0..=3)) & ch(';');
  let schema = Schema::new("Foo").define("A", a);

  let mut events = Vec::new();
  let event_handler = |e: &Event<_, _>| {
    println!("> {:?}", e);
    events.push(e.clone());
  };
  let mut parser = Context::new(&schema, "A", event_handler).unwrap();
  for ch in "A09;".chars() {
    parser.push(ch).unwrap();
  }
  parser.finish().unwrap();
  println!("{:?}", events);
}
