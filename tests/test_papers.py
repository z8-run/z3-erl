"""Paper features, runtime correspondence and adversarial verification examples."""
import json
from pathlib import Path
import subprocess

from case import case, root


class papers(case):
    def test_paper_examples_on_all_backends(self):
        report = self.invoke("check", root / "demo/paper.ex", "--engine", "all", "--timeout", "20000")
        self.assertEqual(len(report["contracts"]), 14)
        self.assertTrue(all(c["correctness"] == "total" for c in report["contracts"]))

    def test_requirements_cannot_skip_a_runtime_clause(self):
        path = self.source("""
        @verifier requires x > 0
        @verifier ensures pick(x) === 1
        defv pick(x) when is_integer(x), do: 1
        @verifier ensures pick(x) === 2
        defv pick(x), do: 2
        @verifier ensures client() === 2
        defv client(), do: pick(0)
        """)
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertEqual({c["owner"]: c["status"] for c in report["contracts"]},
                         {"Fixture.pick/1": "proved", "Fixture.client/0": "unproved"})

    def test_omitted_clauses_and_unerased_proofs_are_rejected(self):
        for text in [
            "def pick(0), do: 9\n@verifier ensures pick(x) === 1\ndef pick(x), do: 1",
            "@verifier ensures pick(x) === 1\ndef pick(x), do: 1\ndef pick(0), do: 9",
            "@verifier ensures pick() === 7\ndef pick() do\n7\nghost do\nassert true\nend\nend",
        ]:
            self.invoke("check", self.source(text), code=2)

    def test_ghost_assumptions_are_reported_and_propagated(self):
        path = self.source("""
        @verifier ensures admitted() === 9
        defv admitted() do
          ghost do
            assume false
          end
          1
        end
        @verifier ensures client() === 9
        defv client(), do: admitted()
        """)
        report = self.invoke("check", path, "--engine", "all", code=1)
        self.assertTrue(all(c["status"] == "conditional" for c in report["contracts"]))
        admitted = next(c for c in report["contracts"] if c["owner"].endswith("admitted/0"))
        self.assertEqual(len(admitted["assumptions"]), 1)
        self.assertTrue(all(e["status"] == "proved" for e in report["evidence"]))

    def test_local_proof_facts_and_havoc_do_not_leak(self):
        path = self.source("""
        defv local() do
          ghost do
            havoc x
            block do
              assume false
            end
            assert false
          end
        end
        defv fresh() do
          ghost do
            havoc x
            y = x
            havoc x
            assert x === y
          end
        end
        """)
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertTrue(all(c["status"] == "unproved" for c in report["contracts"]))

    def test_quantifiers_are_scoped_and_check_definedness(self):
        path = self.source("""
        @verifier ensures same(x) === x
        defv same(x) do
          ghost do
            assert forall x, do: x === x
            assert exists y, do: y === x
          end
          x
        end
        """)
        report = self.invoke("check", path, "--engine", "all")
        plan = json.loads((Path(report["artifacts"]) / "plan.json").read_text())
        self.assertTrue(any('"quant"' in json.dumps(v) for v in plan["conditions"]))
        for expr in ["forall x, do: x === 0", "exists x, do: hd(x) === 1"]:
            path = self.source(f"defv bad() do\nghost do\nassert {expr}\nend\nend")
            self.invoke("check", path, "--engine", "z3", code=1)

    def test_tuple_bounds_and_large_indices(self):
        for expr in ["elem({1, 2}, -1)", "elem({1, 2}, 2)", "elem({1}, true)", "tuple_size([])"]:
            self.invoke("check", self.source(f"defv bad(), do: {expr}"), "--engine", "z3", code=1)
        fields = ",".join(map(str, range(300)))
        path = self.source(f"@verifier ensures last() === 299\ndefv last(), do: elem({{{fields}}}, 299)")
        self.invoke("check", path, "--engine", "z3")

    def test_quantified_tuple_extensionality(self):
        path = self.source("""
        @verifier requires is_tuple(a) and is_tuple(b)
        @verifier requires tuple_size(a) === tuple_size(b)
        @verifier requires forall i, do: not is_integer(i) or i < 0 or i >= tuple_size(a) or elem(a,i) === elem(b,i)
        @verifier ensures same(a,b) === true
        defv same(a,b), do: a === b
        """)
        self.invoke("check", path, "--engine", "all", "--timeout", "20000")
        path = self.source("defv false_quant() do\nghost do\nassert forall x, do: not is_integer(x) or x > 0\nend\nend")
        self.invoke("check", path, "--engine", "z3", code=1)

    def test_ghost_head_patterns_and_assertion_messages(self):
        path = self.source("""
        @verifier ensures swap({x,y}) === {y,x}
        defvg swap({x,y}), do: {y,x}
        defv proof() do
          unfold swap({1,2})
          assert swap({1,2}) === {2,1}, "swapped fields"
        end
        """)
        self.invoke("check", path, "--engine", "all")
        path = self.source('defv bad() do\nassert false, "paper assertion failed"\nend')
        report = self.invoke("check", path, "--engine", "z3", code=1)
        self.assertIn("paper assertion failed", report["evidence"][0]["detail"])

    def test_nested_matches_keep_repeated_variable_constraints(self):
        path = self.source("defv bad() do\n[h | t = [h]] = [1, 2]\nt\nend")
        self.invoke("check", path, "--engine", "z3", code=1)

    def test_lexicographic_measures(self):
        body = """
        @verifier requires is_integer(n) and is_integer(m) and n >= 0 and m >= 0
        @verifier ensures is_integer(walk(n,m)) and walk(n,m) === 0
        @verifier decreases {n,m}
        defvg walk(n,m) do
          if m > 0 do
            walk(n,m-1)
          else
            if n > 0, do: walk(n-1,2), else: 0
          end
        end
        """
        self.invoke("check", self.source(body), "--engine", "all", "--timeout", "20000")
        self.invoke("check", self.source(body.replace("walk(n,m-1)", "walk(n,m)")), "--engine", "z3", code=1)

    def test_paper_demo_runtime_and_ghost_return_erasure(self):
        script = self.dir / "runtime.exs"
        script.write_text('Code.require_file("front/lib/vex.ex")\nCode.require_file("demo/paper.ex")\n'
                          'IO.puts(JSON.encode!([Paper.proof(), Paper.nested(), Paper.fallback(), '
                          'Paper.uses_dup(9), Paper.fetch({1, 2, 3}, 2)] |> Enum.map(&inspect/1)))\n')
        result = subprocess.run(["elixir", "--erl", "+S 2:2", str(script)], cwd=root,
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(result.stdout), ["7", "{1, [2, 3]}", ":other", "18", "3"])

    def test_proof_names_remain_ordinary_variables(self):
        names = ["ghost", "assert", "unfold", "assume", "havoc", "block"]
        functions = [f"@verifier ensures keep_{name}_{kind}({name}) === {name}\n"
                     f"{kind} keep_{name}_{kind}({name}) do\n0\n{name}\nend"
                     for name in names for kind in ["defv", "def"]]
        path = self.source("\n".join(functions))
        self.invoke("check", path, "--engine", "all")
        calls = [f"Fixture.keep_{name}_{kind}(7)" for name in names for kind in ["defv", "def"]]
        script = self.dir / "runtime.exs"
        script.write_text('Code.require_file("front/lib/vex.ex")\n'
                          'Code.require_file(hd(System.argv()))\n'
                          'IO.puts(JSON.encode!([' + ','.join(calls) + ']))\n')
        result = subprocess.run(["elixir", "--erl", "+S 2:2", str(script), str(path)], cwd=root,
                                capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(result.stdout), [7] * len(calls))
