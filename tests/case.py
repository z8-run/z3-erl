"""Shared real-tool test harness."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

root = Path(__file__).resolve().parents[1]
binary = root / "target/debug/vex"

class case(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="vex-test-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)

    def invoke(self, *args, code=0):
        # Cold Lean startup can exceed the SMT budget after large proof runs.
        if "--timeout" not in args and any(
            a == "--engine" and b in ("lean", "all") for a, b in zip(args, args[1:])
        ):
            args += ("--timeout", "30000")
        result = subprocess.run(
            [str(binary), *map(str, args), "--out", str(self.dir / "out"), "--json"],
            cwd=root, capture_output=True, text=True, timeout=300,
        )
        self.assertEqual(result.returncode, code, result.stdout + result.stderr)
        return json.loads(result.stdout) if result.stdout.startswith("{") else result

    def source(self, body, name="fixture.ex"):
        path = self.dir / name
        path.write_text("defmodule Fixture do\n use :vex\n" + body + "\nend\n")
        return path
