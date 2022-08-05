# How to Profile Terp

## Benchmark

```
$ cargo +nightly bench
```

## Profiling

```shell
$ cargo clean
$ cargo +nightly build --release
$ target/release/terp-prof bench ../src/parser/test/data/*.json*
$ sudo perf stat -- target/release/terp-prof parse ../src/parser/test/data/ok-sgml.json.txt
$ sudo perf record -g -F max -- target/release/terp-prof parse ../src/parser/test/data/ok-sgml.json.txt
$ sudo perf report
$ sudo perf script | perl stackcollapse-perf.pl | perl flamegraph.pl --title "terp" > flamegraph_terp-`git describe --always`.svg
```
