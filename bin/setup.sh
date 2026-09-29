#!/bin/sh
set -eu
vex_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$vex_root"
for tool in cargo elixir z3 dotnet java elan python3 curl rg; do
  command -v "$tool" >/dev/null || { echo "missing prerequisite: $tool" >&2; exit 2; }
done
mkdir -p .tools
if ! dotnet tool list --tool-path .tools/boogie | rg -q '^boogie[[:space:]]+3\.5\.6[[:space:]]'; then
  dotnet tool update boogie --version 3.5.6 --tool-path .tools/boogie
fi
if ! test -f .tools/tla2tools.jar; then
  curl -fsSL https://github.com/tlaplus/tlaplus/releases/download/v1.7.4/tla2tools.jar -o .tools/tla2tools.jar
fi
python3 - <<'PY'
import hashlib
from pathlib import Path
p = Path('.tools/tla2tools.jar')
expected = '936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88'
if hashlib.sha256(p.read_bytes()).hexdigest() != expected:
    raise SystemExit('TLC checksum mismatch; expected the upstream v1.7.4 jar')
PY
elan toolchain install leanprover/lean4:v4.28.0
cargo build --locked --workspace
lake +leanprover/lean4:v4.28.0 build
target/debug/vex doctor
