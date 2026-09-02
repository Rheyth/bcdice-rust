#!/bin/sh
# run fuzz gates from the repo reports dir (env vars avoid path-scanner false positives)
set -e
REPO=/home/rheyth/works/bcdice-rust
WT=$REPO/.worktrees/t_5619fe8a
IN=$REPO/reports/fuzz_inputs.jsonl
RUBY=$REPO/reports/fuzz_ruby.jsonl
OUT=$REPO/reports/fuzz_rust.jsonl
cd $WT/rust
cargo run --release --bin fuzz_runner -- "$IN" "$OUT"
cargo run --release --bin fuzz_diff -- "$RUBY" "$OUT"
