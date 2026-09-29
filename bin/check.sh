#!/bin/sh
set -eu
vex_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$vex_root"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --locked --workspace
lake +leanprover/lean4:v4.28.0 build
(
  cd front
  ERL_FLAGS='+S 2:2' mix format --check-formatted
  ERL_FLAGS='+S 2:2' mix test
)
target/debug/vex doctor
python3 -m unittest discover -s tests -v
target/debug/vex check --config vex.toml
