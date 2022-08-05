#![feature(test)]
extern crate test;

use clap::{Parser, Subcommand};
use std::fs;
use terp::parser::Context;
use terp::schema::json::{schema, ID};
use test::bench::iter;
use test::stats::Summary;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  #[clap(subcommand)]
  subcommand: Commands,
}

#[derive(Debug, Clone, Subcommand)]
enum Commands {
  Bench {
    #[clap(short, long, value_parser)]
    files: Vec<String>,
  },
}

fn main() {
  let args = Args::parse();
  match &args.subcommand {
    Commands::Bench { files } => {
      if files.is_empty() {
        eprintln!("ERROR: JSON files are not specified.");
      }
      for file in files {
        bench(&file);
      }
    }
  }
}

fn bench(filename: &str) {
  let content = fs::read_to_string(filename).unwrap();
  println!("[{}: {} chars]", filename, num(content.chars().count() as u64));
  bench_report("  terp", bench_terp(&content));
  bench_report("  terp (naive)", bench_terp_naive(&content));
  bench_report("  nom", bench_nom(&content));
  bench_report("  serde_json", bench_serde_json(&content));
}

fn bench_terp(content: &str) -> Summary {
  let schema = schema();
  iter(&mut || {
    let mut parser = Context::new(&schema, ID::JsonText, |_| ()).unwrap();
    parser.push_str(content).unwrap();
    parser.finish().unwrap();
  })
}

fn bench_terp_naive(content: &str) -> Summary {
  let schema = naive_schema();
  iter(&mut || {
    let mut parser = Context::new(&schema, ID::JsonText, |_| ()).unwrap();
    parser.push_str(content).unwrap();
    parser.finish().unwrap();
  })
}

fn bench_nom(content: &str) -> Summary {
  iter(&mut || {
    terp_prof::nom::json_text(content).unwrap();
  })
}

fn bench_serde_json(content: &str) -> Summary {
  iter(&mut || {
    serde_json::from_str::<serde_json::Value>(content).unwrap();
  })
}

fn bench_report(target: &str, summary: Summary) {
  println!("{target:15}: {:>10} ns/iter (±{:.1}%)", num(summary.median as u64), summary.median_abs_dev_pct);
}

fn num(n: u64) -> String {
  let mut s = n.to_string();
  let mut i = s.len();
  while i > 3 {
    i -= 3;
    s.insert(i, ',');
  }
  s
}

pub fn naive_schema() -> terp::schema::Schema<ID, char> {
  use terp::schema::chars::{ch, token};
  use terp::schema::json::ID::*;
  use terp::schema::{id, one_of, range, Schema};
  Schema::new("JSON")
    .define(JsonText, id(WS) & id(Value) & id(WS))
    .define(BeginArray, id(WS) & ch('[') & id(WS))
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
    .define(Char, id(Unescaped) | id(Escape) & (one_of(&['\"', '\\', '/', 'b', 'f', 'n', 'r', 't']) | (id(HexDig) * 4)))
    .define(Escape, ch('\\'))
    .define(QuotationMark, ch('\"'))
    .define(Unescaped, range('\x20'..='\x21') | range('\x23'..='\x5B') | range('\x5D'..='\u{10FFFF}'))
    .define(Digit, range('0'..='9'))
    .define(HexDig, range('0'..='9') | range('a'..='f') | range('A'..='F'))
}
