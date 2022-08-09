# Terp Reference

The original repository is [github.com/torao/terp](https://github.com/torao/terp).

## Notation

| Notation | Meaning |
|:---|:---|
| `'X'` | 文字 `X` |
| `?` | 0 または 1 回の繰り返し |
| `*` | 0 回以上の繰り返し |
| `+` | 1 回以上の繰り返し |
| `{X,Y}` | X 回以上 Y 回以下の繰り返し |
| `{X,}` | X 回以上の繰り返し |
| `{,Y}` | Y 回以下の繰り返し |
| `A & B` | A に続く B の連続 |
| `A \| B` | A または B |
| `(A ..)` | A .. のグルーピング |

上限のない量指定子の実装上の上限は `usize::MAX` である。

繰り返しの表記が省略されている構文は暗黙的に 1 回の出現 (最小 1 回、最大 1 回の繰り返し) を示している。

## 用語

### Matcher

シーケンスとの一致を判定する処理。アイテムのあるバッファに対して先頭からの一致を判定し、一致確定 `Match(length:usize)`、不一致確定 `Unmatch`、あるいはより多くのアイテムが到着しないと判断できないことを示す `More` を返す。

アプリケーションは `Matcher` を実装することで

Matcher のインスタンスは冪等でなければならない。

## 処理の流れ

C/C++ や Java に似た以下のような文字列リテラルのスキーマを想定する。


## Dive Inside Terp

The following pages provide useful information for developers and source code readers.

* [Terp v0.1](inside-v0.1.md)
