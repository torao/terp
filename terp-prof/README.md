# How to Profile Terp

## Benchmark

```
$ cargo +nightly bench
```

## Profiling

```
$ cargo clean
$ cargo +nightly build --release
$ sudo perf stat -- target/release/terp-prof
$ sudo perf record -g -- target/release/terp-prof
$ sudo perf script | perl stackcollapse-perf.pl | perl flamegraph.pl --title "terp" > flamegraph_terp.svg
```
