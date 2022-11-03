//! Tests of how to build a parser that accepts context-free grammars.
//!
use crate::parser::{Context, Event};
use crate::schema::chars::token;
use crate::schema::{id, Schema};

/// 𝐿={𝑎ⁿ𝑏ⁿ|𝑛≧1}
#[test]
fn 𝑎ⁿ𝑏ⁿ() {
  let a = (token("a") & id("A") & token("b")) | token("ab");
  let schema = Schema::new("Foo").define("A", a);

  for n in 1..10 {
    let mut events = Vec::new();
    let handler = |e: &Event<_, _>| events.push(e.clone());
    let mut parser = Context::new(&schema, "A", handler).unwrap();

    let sample = (0..n).fold("ab".to_string(), |ab, _| format!("a{}b", ab));
    parser.push_str(&sample).unwrap();
    parser.finish().unwrap();
    println!("{:?}", events);
  }
}
