#[test]
fn parser_behavior() {
  use crate::parser::{Context, Event, EventKind};
  use crate::schema::chars::{one_of_chars, Location};
  use crate::schema::{id, Schema};

  let schema = Schema::new("Trump")
    .define("CARD", id("SUIT") & id("RANK"))
    .define("SUIT", one_of_chars("♠♣♦♥"))
    .define("RANK", one_of_chars("A233456789XJQK"));

  let mut events = Vec::new();
  let handler = |e: &Event<_, _>| events.push(e.clone());
  let mut parser = Context::new(&schema, "CARD", handler).unwrap();
  parser.push_str("♠2").unwrap();
  parser.finish().unwrap();

  assert_eq!(
    vec![
      Event { location: Location { chars: 0, lines: 0, columns: 0 }, kind: EventKind::Begin("CARD") },
      Event { location: Location { chars: 0, lines: 0, columns: 0 }, kind: EventKind::Begin("SUIT") },
      Event { location: Location { chars: 0, lines: 0, columns: 0 }, kind: EventKind::Fragments(vec!['♠']) },
      Event { location: Location { chars: 1, lines: 0, columns: 1 }, kind: EventKind::End("SUIT") },
      Event { location: Location { chars: 1, lines: 0, columns: 1 }, kind: EventKind::Begin("RANK") },
      Event { location: Location { chars: 1, lines: 0, columns: 1 }, kind: EventKind::Fragments(vec!['2']) },
      Event { location: Location { chars: 2, lines: 0, columns: 2 }, kind: EventKind::End("RANK") },
      Event { location: Location { chars: 2, lines: 0, columns: 2 }, kind: EventKind::End("CARD") },
    ],
    events
  );

  let handler = |e: &Event<_, _>| println!("{:?}", e.kind);
  let mut parser = Context::new(&schema, "CARD", handler).unwrap();
  println!("-- pushing ♠ --");
  parser.push_str("♠").unwrap();
  println!("-- pushing 2 --");
  parser.push_str("2").unwrap();
  println!("-- finishing --");
  parser.finish().unwrap();
}
