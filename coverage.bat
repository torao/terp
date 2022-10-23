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

rem see also: .github/actions-rs/grcov.yml
grcov target/debug/prof -s . --binary-path ./target/debug/ -t html --ignore-not-existing -o ./target/debug/coverage/ --excl-line "#\[derive\(" --excl-br-line "#\[derive\(" --ignore "src/main.rs" --ignore "**/test.rs" --ignore "**/test/*"
start .\target\debug\coverage\index.html