@echo off
rem rustup update nightly
rem rustup component add llvm-tools-preview
rem 

cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

rem del prof\* /Q
rem cargo clean

setlocal
set "RUSTFLAGS=-C instrument-coverage"
set "LLVM_PROFILE_FILE=target/debug/prof/terp-%%p-%%m.profraw"
set "RUST_LOG=debug"
cargo +nightly test %*

grcov target/debug/prof -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o ./target/debug/coverage/ --excl-line "#\[derive\(" --excl-br-line "#\[derive\(" 
start .\target\debug\coverage\index.html