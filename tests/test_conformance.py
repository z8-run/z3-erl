"""Compare the shared encoding with actual OTP integer operations."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

root = Path(__file__).resolve().parents[1]


class conformance(unittest.TestCase):
    def test_containers_against_otp(self):
        expressions = [f"is_list({v})" for v in ["0", "true", "nil", "[]", "[1]", "[1|false]", "{}", "{1,2}"]]
        expressions += [f"tuple_size({v})" for v in ["{}", "{1}", "{1, false, [2|3]}"]]
        expressions += [f"elem({{1, false, [2|3]}}, {i})" for i in range(3)]
        expressions += ["hd([1|false])", "tl([1|false])", "tl([1, 2])", "{} === {}", "{1} === {1,2}"]
        with tempfile.TemporaryDirectory(prefix="vex-containers-") as directory:
            directory = Path(directory)
            inputs = directory / "input.json"
            inputs.write_text(json.dumps(expressions))
            oracle = directory / "oracle.exs"
            oracle.write_text('System.argv() |> hd() |> File.read!() |> JSON.decode!() '
                              '|> Enum.map(fn text -> {value, _} = Code.eval_string(text); '
                              'inspect(value, limit: :infinity) end) |> JSON.encode!() |> IO.puts()')
            actual = subprocess.run(["elixir", "--erl", "+S 2:2", str(oracle), str(inputs)],
                                    capture_output=True, text=True, check=True, timeout=30)
            functions = [f"@verifier ensures sample_{i}() === ({expected})\ndefv sample_{i}(), do: {expr}"
                         for i, (expr, expected) in enumerate(zip(expressions, json.loads(actual.stdout)))]
            source = directory / "containers.ex"
            source.write_text("defmodule Containers do\nuse :vex\n" + "\n".join(functions) + "\nend")
            result = subprocess.run([str(root / "target/debug/vex"), "check", str(source), "--engine", "all",
                                     "--out", str(directory / "out"), "--timeout", "20000", "--json"],
                                    cwd=root, capture_output=True, text=True, timeout=180)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(len(json.loads(result.stdout)["contracts"]), len(expressions))

    def test_integer_profile_against_otp(self):
        pairs = [(-7, 3), (7, -3), (-7, -3), (0, 3), (7, 3), (-2, 5), (2, -5),
                 (10**45 + 17, 13), (-10**45 - 17, 13)]
        rows = [{"op": op, "args": [str(a), str(b)]}
                for a, b in pairs for op in ["div", "rem", "+", "-", "*"]]
        with tempfile.TemporaryDirectory(prefix="vex-conformance-") as directory:
            directory = Path(directory)
            inputs = directory / "oracle.json"
            inputs.write_text(json.dumps(rows))
            actual = subprocess.run(["elixir", "--erl", "+S 2:2", str(root / "front/oracle.exs"), str(inputs)],
                                    capture_output=True, text=True, check=True, timeout=30)
            values = json.loads(actual.stdout)
            functions = []
            for i, (row, expected) in enumerate(zip(rows, values)):
                a, b = row["args"]
                expr = f'{row["op"]}({a}, {b})' if row["op"] in ["div", "rem"] else f'({a}) {row["op"]} ({b})'
                functions.append(f'@verifier ensures sample_{i}() === ({expected})\ndefv sample_{i}(), do: {expr}')
            source = directory / "conformance.ex"
            source.write_text("defmodule Conformance do\nuse :vex\n" + "\n".join(functions) + "\nend\n")
            result = subprocess.run([str(root / "target/debug/vex"), "check", str(source), "--engine", "all", "--jobs", "4",
                                     "--out", str(directory / "out"), "--timeout", "20000", "--json"],
                                    cwd=root, capture_output=True, text=True, timeout=240)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(len(report["contracts"]), len(rows))
            self.assertTrue(all(e["status"] == "proved" for e in report["evidence"]))


if __name__ == "__main__":
    unittest.main()
