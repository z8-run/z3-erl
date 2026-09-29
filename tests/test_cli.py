"""Real-tool regressions. Run after bin/setup.sh with python3 -m unittest discover -s tests."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

root = Path(__file__).resolve().parents[1]
binary = root / "target/debug/vex"
lean = "leanprover/lean4:v4.28.0"


class cli(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="vex-test-")
        self.addCleanup(self.tmp.cleanup)
        self.dir = Path(self.tmp.name)

    def invoke(self, *args, code=0):
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

    def test_basic_all_backends(self):
        report = self.invoke("check", root / "demo/basic.ex", "--engine", "all", "--timeout", "20000")
        self.assertTrue(report["success"])
        self.assertEqual({e["engine"] for e in report["evidence"]}, {"z3", "boogie", "lean"})

    def test_fibonacci_modular_total_correctness(self):
        report = self.invoke("check", root / "demo/fib.ex", "--engine", "all", "--timeout", "20000")
        self.assertEqual(len(report["contracts"]), 3)
        self.assertTrue(all(c["correctness"] == "total" for c in report["contracts"]))

    def test_false_postcondition_and_model_are_retained(self):
        report = self.invoke("check", root / "demo/fail/post.ex", "--engine", "z3", code=1)
        self.assertIn("counterexample", {e["status"] for e in report["evidence"]})
        self.assertTrue(list(Path(report["artifacts"]).glob("vc/*.model.log")))
        report = self.invoke("check", root / "demo/fail/post.ex", "--engine", "boogie", code=1)
        self.assertIn("counterexample", {e["status"] for e in report["evidence"]})

    def test_source_paths_cannot_inject_solver_commands(self):
        path = self.source(
            "@verifier ensures one() === 2\ndefv one(), do: 1",
            "file\r(assert false)\r;.ex",
        )
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertEqual(report["evidence"][0]["status"], "counterexample")

    def test_dependencies_cannot_hide_a_failed_callee(self):
        path = self.source("""
        @verifier requires is_integer(n)
        @verifier ensures bad(n) === n + 1
        defv bad(n), do: n - 1
        @verifier requires is_integer(n)
        @verifier ensures client(n) === n + 1
        defv client(n), do: bad(n)
        """)
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertTrue(all(c["status"] == "unproved" for c in report["contracts"]))

    def test_module_identity_matches_the_runtime_namespace(self):
        path = self.dir / "modules.ex"
        helper = "defmodule Helper do\nuse :vex\n@verifier ensures value() === 1\ndefv value(), do: 1\nend\n"
        for target, code in [("Elixir.Helper", 0), (':"Elixir.Helper"', 0), (':"Helper"', 2)]:
            path.write_text(helper + "defmodule Fixture do\nuse :vex\n"
                f"@verifier ensures answer() === 1\ndefv answer(), do: {target}.value()\nend\n")
            self.invoke("check", path, "--engine", "z3", code=code)
        path.write_text(helper + "defmodule Elixir.Helper do\nuse :vex\ndefv value(), do: 2\nend\n")
        result = self.invoke("check", path, code=2)
        self.assertIn("duplicate module", result.stderr)

    def test_arithmetic_safety_and_case_coverage(self):
        path = self.source("""
        @verifier requires is_integer(n)
        @verifier ensures zero(n) === 0
        defv zero(n), do: div(n, 0)
        @verifier requires is_integer(n)
        @verifier ensures incomplete(n) === 0
        defv incomplete(n) do
          case n do
            0 -> 0
          end
        end
        """)
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertTrue(all(c["status"] == "unproved" for c in report["contracts"]))

    def test_short_circuit_and_truthiness(self):
        path = self.source("""
        @verifier ensures skip() === false
        defv skip(), do: false and div(1, 0)
        @verifier ensures truth() === 7
        defv truth(), do: if(0, do: 7, else: 9)
        @verifier ensures value() === 4
        defv value(), do: true and 4
        @verifier ensures nils() === false
        defv nils(), do: nil === []
        @verifier ensures tails() === false
        defv tails(), do: [1 | nil] === [1 | false]
        @verifier ensures proper() === false
        defv proper(), do: [1 | nil] === [1]
        """)
        self.invoke("check", path, "--engine", "all", "--timeout", "20000")

    def test_assertions_are_checked_before_becoming_facts(self):
        path = self.source("""
        @verifier requires is_integer(n)
        @verifier ensures claim(n) === 900
        defv claim(n) do
          ghost do
            assert false
          end
          0
        end
        """)
        self.invoke("check", path, "--engine", "z3", code=1)

    def test_erased_bindings_do_not_change_runtime_scope(self):
        path = self.source("""
        @verifier requires is_integer(n)
        @verifier ensures same(n) === n
        defv same(n) do
          ghost do
            n = 900
            assert n === 900
          end
          n
        end
        """)
        self.invoke("check", path, "--engine", "z3")

    def test_recursive_ghost_cannot_introduce_inconsistent_axioms(self):
        path = self.source("""
        @verifier requires is_integer(n) and n >= 0
        @verifier ensures is_integer(bad(n))
        defvg bad(n), do: bad(n) + 1
        @verifier requires is_integer(n) and n >= 0
        @verifier ensures broken(n) === 1
        defv broken(n) do
          unfold bad(n)
          0
        end
        """)
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertTrue(all(c["status"] == "unproved" for c in report["contracts"]))

    def test_erased_code_cannot_use_a_partial_runtime_summary(self):
        path = self.source("""
        @verifier ensures false
        defvp loop(), do: loop()
        @verifier ensures false
        defv returns() do
          ghost do
            loop()
          end
          0
        end
        """)
        result = self.invoke("check", path, code=2)
        self.assertIn("erased proof code", result.stderr)

    def test_unfold_cannot_bootstrap_its_own_recursive_contract(self):
        path = self.source("""
        @verifier requires is_integer(n) and n >= 1
        @verifier ensures false
        defvg bad(n) do
          if n === 1 do
            unfold bad(2)
            0
          else
            bad(n - 1)
          end
        end
        """)
        result = self.invoke("check", path, code=2)
        self.assertIn("own recursive component", result.stderr)

    def test_expression_bindings_and_erlang_names_are_not_misinterpreted(self):
        for expression in ["{n = 1, n}", "(n = 1) + n", "true && (n = 1)"]:
            path = self.source(f"@verifier ensures true\ndefv scope(n), do: {expression}")
            result = self.invoke("check", path, code=2)
            self.assertIn("bindings must be statements", result.stderr)
        for expression in [":erlang.elem({1}, 0)", ":erlang.and(false, div(1, 0))"]:
            path = self.source(f"@verifier ensures true\ndefv invalid(), do: {expression}")
            self.invoke("check", path, code=2)
        path = self.source("@verifier ensures value() === -2\ndefv value(), do: :erlang.div(-7, 3)")
        self.invoke("check", path, "--engine", "z3")

    def test_termination_is_not_inferred_from_partial_recursion(self):
        path = self.source("""
        @verifier requires is_integer(n)
        @verifier ensures loop(n) === 7
        defv loop(n), do: loop(n)
        """)
        report = self.invoke("check", path, "--engine", "z3")
        self.assertEqual(report["contracts"][0]["correctness"], "partial")

    def test_unsupported_effects_and_empty_scope_fail(self):
        path = self.source("@verifier ensures send_it(p) === :ok\ndefv send_it(p), do: send(p, :ok)")
        self.invoke("check", path, code=2)
        path = self.source("import Custom\n@verifier ensures one() === 1\ndefv one(), do: 1", "macro.ex")
        self.invoke("check", path, code=2)
        path = self.source("def ordinary(), do: :ok", "empty.ex")
        self.invoke("check", path, code=2)

    def test_strict_inventory(self):
        path = self.source("@verifier ensures one() === 1\ndefv one(), do: 1\ndef ordinary(), do: :ok")
        report = self.invoke("check", path, "--engine", "z3")
        self.assertEqual(len(report["unchecked"]), 1)
        self.invoke("check", path, "--strict", code=2)

    def test_missing_engine_cannot_pass(self):
        path = self.source("@verifier ensures one() === 1\ndefv one(), do: 1")
        cfg = self.dir / "vex.toml"
        cfg.write_text(f'sources = [{json.dumps(str(path))}]\nengine = "z3"\n[tools]\nz3 = "/does/not/exist"\n')
        report = self.invoke("check", "--config", cfg, code=1)
        self.assertEqual(report["evidence"][0]["status"], "unavailable")

    def test_tlc_positive_negative_and_source_export(self):
        mailbox = self.invoke("model", root / "demo/mailbox/mailbox.tla")
        self.assertEqual(mailbox["models"][0]["status"], "model_checked")
        unfair = self.invoke("model", root / "demo/mailbox/mailbox.tla",
            "--cfg", root / "demo/mailbox/unfair.cfg", code=1)
        self.assertEqual(unfair["models"][0]["status"], "counterexample")
        bad = self.invoke("model", root / "demo/fail/race.tla", code=1)
        self.assertEqual(bad["models"][0]["status"], "counterexample")
        good = self.invoke("check", "--config", root / "demo/counter/vex.toml")
        self.assertTrue(good["success"])
        self.assertTrue((Path(good["artifacts"]) / "models/0/vex.tla").is_file())

    def test_source_exports_cannot_invent_runtime_calls(self):
        sources = [
            "defmodule Fixture do\nuse :vex\ndefvg step(n), do: n\nend",
            "defmodule Secret do\nuse :vex\ndefvp hidden(n), do: n\nend\n"
            "defmodule Fixture do\nuse :vex\ndefv step(n), do: Secret.hidden(n)\nend",
        ]
        path = self.dir / "export.ex"
        cfg = self.dir / "vex.toml"
        cfg.write_text(
            'sources = ["export.ex"]\n[[models]]\n'
            f'file = {json.dumps(str(root / "demo/counter/counter.tla"))}\n'
            '[models.exports]\nstep = "Fixture.step/1"\n'
        )
        for source in sources:
            path.write_text(source)
            result = self.invoke("model", "--config", cfg, code=2)
            self.assertIn("vex:", result.stderr)

    def test_lean_rejects_sorry_and_extra_axioms(self):
        for text in ["theorem hole : False := by sorry", "axiom fake : False\ntheorem hole : False := fake"]:
            path = self.dir / "hole.lean"
            path.write_text("import kernel.lean.audit\n" + text + "\n#vex_audit hole\n")
            result = subprocess.run(["lake", "+" + lean, "env", "lean", str(path)], cwd=root, capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("vex rejected axiom", result.stdout)

    def test_custom_proof_terms_are_checked_against_generated_statements(self):
        path = self.source("@verifier ensures one() === 1\ndefv one(), do: 1")
        report = self.invoke("check", path, "--engine", "z3")
        proof_dir = self.dir / "proofs"
        proof_dir.mkdir()
        condition = report["evidence"][0]["id"]
        proof = proof_dir / (condition + ".lean")
        proof.write_text("by trivial\n")
        self.invoke("check", path, "--engine", "lean", "--proofs", proof_dir)
        proof.write_text("by sorry\n")
        bad = self.invoke("check", path, "--engine", "lean", "--proofs", proof_dir, code=1)
        self.assertEqual(bad["evidence"][0]["status"], "error")

    def test_lean_pin_is_exact(self):
        self.assertEqual((root / "lean-toolchain").read_text().strip(), lean)
        report = self.invoke("doctor")
        self.assertIn("version 4.28.0,", report["lean"])


if __name__ == "__main__":
    unittest.main()
