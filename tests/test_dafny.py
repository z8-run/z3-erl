"""Dafny translations: real proofs, executable behavior, and rejected mutations."""
import itertools
import json
import subprocess

from case import case, root

demo = root / "demo/dafny"


class dafny(case):
    def reject(self, name, text, *deps):
        path = self.dir / f"{name}.ex"
        path.write_text(text)
        return self.invoke("check", *(demo / f"{n}.ex" for n in deps), path,
                           "--engine", "z3", "--timeout", "1000", "--jobs", "4", code=1)

    def mutate(self, name, before, after, *deps):
        text = (demo / f"{name}.ex").read_text()
        self.assertIn(before, text)
        return self.reject(name, text.replace(before, after), *deps)

    def unproved(self, report, owner):
        self.assertEqual(next(c["status"] for c in report["contracts"] if c["owner"] == owner),
                         "unproved")

    def verify(self, *names, engine="z3"):
        report = self.invoke("check", *(demo / f"{n}.ex" for n in names),
                             "--engine", engine, "--timeout", "10000", "--jobs", "4")
        self.assertTrue(report["contracts"])
        self.assertTrue(all(c["status"] == "proved" and c["correctness"] == "total"
                            and not c["assumptions"] for c in report["contracts"]))
        return report

    def runtime(self, names, body):
        script = self.dir / "runtime.exs"
        script.write_text('Enum.each(System.argv(), &Code.require_file/1)\n' + body)
        result = subprocess.run(["elixir", "--erl", "+S 2:2", str(script),
                                 str(root / "front/lib/vex.ex"),
                                 *(str(demo / f"{n}.ex") for n in names)],
                                cwd=root, capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        return json.loads(result.stdout)

    def test_maximum(self):
        for engine in ["z3", "boogie"]:
            self.verify("max", engine=engine)
        arrays = [[-7], [-9, -3, -12], [4, 4, 2], [0, 9, 2, 11, -4], [10**40, 1]]
        values = self.runtime(["max"], 'xs = ' + repr(arrays) + '\n'
                              'IO.puts(JSON.encode!(Enum.map(xs, fn xs -> :max.find(List.to_tuple(xs)) end)))\n')
        self.assertEqual(values, [max(xs) for xs in arrays])

    def test_maximum_rejects_reversed_comparison(self):
        report = self.mutate("max", "if m < x", "if m > x")
        self.unproved(report, ":max.find/1")

    def test_lists(self):
        for engine in ["z3", "boogie"]:
            self.verify("list", engine=engine)
        values = self.runtime(["list"], '''
        pairs = [{[], []}, {[], [1]}, {[1, 2], []}, {[1, 2], [3, 4]},
                 {[[3], :ok], [{4, 5}, nil]}]
        IO.puts(JSON.encode!(Enum.map(pairs, fn {xs, ys} -> :seq.append(xs, ys) === xs ++ ys end)))
        ''')
        self.assertEqual(values, [True] * 5)

    def test_lists_reject_missing_tail(self):
        report = self.mutate("list", "zs = append(t, ys)", "zs = ys")
        self.unproved(report, ":seq.append/2")

    def test_environment(self):
        for engine in ["z3", "boogie"]:
            self.verify("env", engine=engine)
        values = self.runtime(["env"], '''
        IO.puts(JSON.encode!([:env.get({}, 0), :env.get({4, 9, -2}, -1),
                             :env.get({4, 9, -2}, 0), :env.get({4, 9, -2}, 2),
                             :env.get({4, 9, -2}, 3), function_exported?(:env, :valid, 1)]))
        ''')
        self.assertEqual(values, [0, 0, 4, -2, 0, False])

    def test_quantified_ghosts_check_definedness_and_erasure(self):
        body = """
        @verifier ensures all() === true
        defvg all(), do: (forall x, do: x === x)
        @verifier ensures kept(n) === n
        defv kept(n) do
          ghost do
            q = exists x, do: x === n
            assert q === true
          end
          n
        end
        """
        self.invoke("check", self.source(body), "--engine", "all", "--timeout", "30000")
        bad = "defvg bad(), do: (forall x, do: hd(x) === 0)"
        self.invoke("check", self.source(bad), "--engine", "z3", code=1)
        runtime = "defv bad(), do: (forall x, do: x === x)"
        rejected = self.invoke("check", self.source(runtime), "--engine", "z3", code=2)
        self.assertIn("quantifiers belong in contracts and ghost code", rejected.stderr)

    def test_quantified_values_reject_unchecked_recursion(self):
        direct = "@verifier decreases term_size(n)\ndefvg loop(n), do: (forall x, do: loop(n) !== true)"
        rejected = self.invoke("check", self.source(direct), "--engine", "z3", code=2)
        self.assertIn("quantified values require total calls", rejected.stderr)
        mutual = """
        @verifier decreases term_size(n)
        defvg left(n), do: (exists x, do: right(n) === true)
        @verifier decreases term_size(n)
        defvg right(n), do: left(n)
        """
        rejected = self.invoke("check", self.source(mutual), "--engine", "z3", code=2)
        self.assertIn("quantified values require total calls", rejected.stderr)
        self.invoke("check", self.source("defvg bad(), do: (exists x, do: 7)"),
                    "--engine", "z3", code=1)

    def test_expressions(self):
        for engine in ["z3", "boogie"]:
            self.verify("env", "expr", engine=engine)
        values = self.runtime(["env", "expr"], '''
        leaf = [{:const, -3}, {:const, 0}, {:const, 100000000000000000000},
                {:var, -1}, {:var, 0}, {:var, 1}, {:var, 7}]
        trees = leaf ++ for op <- [:add, :sub], a <- leaf, b <- leaf, do: {op, a, b}
        oracle = fn self, e, ctx ->
          case e do
            {:const, n} -> n
            {:var, i} -> if i >= 0 and i < tuple_size(ctx), do: elem(ctx, i), else: 0
            {:add, a, b} -> self.(self, a, ctx) + self.(self, b, ctx)
            {:sub, a, b} -> self.(self, a, ctx) - self.(self, b, ctx)
          end
        end
        deep = Enum.reduce(1..40, {:var, 0}, fn i, e -> {:sub, {:const, i}, e} end)
        results = for e <- [deep | trees], ctx <- [{}, {9, -5}] do
          :expr.eval(e, ctx) === oracle.(oracle, e, ctx)
        end
        IO.puts(JSON.encode!(results))
        ''')
        self.assertEqual(values, [True] * 212)

    def test_expressions_reject_wrong_subtraction(self):
        report = self.mutate("expr", "eval(a, ctx) - eval(b, ctx)", "eval(a, ctx) + eval(b, ctx)", "env")
        self.unproved(report, ":expr.eval/2")

    def test_rewriting(self):
        for engine in ["z3", "boogie"]:
            self.verify("env", "expr", "rewrite", engine=engine)
        values = self.runtime(["env", "expr", "rewrite"], '''
        z = {:const, 0}
        v = {:var, 0}
        cases = [{{:add, z, v}, v}, {{:add, v, z}, v}, {{:sub, v, z}, v},
                 {{:sub, z, z}, z}, {{:sub, z, v}, {:sub, z, v}},
                 {{:add, {:sub, v, z}, {:add, z, z}}, v}]
        rules = Enum.map(cases, fn {e, expected} -> :rewrite.run(e) === expected end)
        leaf = [z, v, {:var, 5}, {:const, -9}]
        trees = for op <- [:add, :sub], a <- leaf, b <- leaf, do: {op, a, b}
        deep = Enum.reduce(1..30, v, fn _, e -> {:add, {:sub, e, z}, z} end)
        semantics = for e <- [deep | trees], ctx <- [{}, {7}, {-3, 4}] do
          :expr.eval(:rewrite.run(e), ctx) === :expr.eval(e, ctx)
        end
        IO.puts(JSON.encode!(rules ++ semantics))
        ''')
        self.assertEqual(values, [True] * 105)

    def test_rewriting_rejects_zero_minus_rule(self):
        report = self.mutate("rewrite", "op === :add and a === {:const, 0}", "a === {:const, 0}",
                             "env", "expr")
        self.unproved(report, ":rewrite.correct/2")

    def test_stack_machine(self):
        for engine in ["z3", "boogie"]:
            self.verify("env", "list", "stack", engine=engine)
        values = self.runtime(["env", "list", "stack"], '''
        results = [:stack.step(:add, [], {}), :stack.step(:sub, [4], {}),
                   :stack.step(:sub, [3, 10, 99], {}),
                   :stack.exec([:sub, {:const, 3}, {:const, 10}], [40], {}),
                   :stack.exec([:sub, {:var, 0}, :add, {:const, 7}, {:const, 2}], [], {4}),
                   :stack.exec([{:var, 9}], [5], {4}),
                   :stack.exec([], [7, 8], {})]
        IO.puts(JSON.encode!(results))
        ''')
        self.assertEqual(values, [[], [4], [7, 99], [7, 40], [5], [0, 5], [7, 8]])

    def test_stack_rejects_reversed_operands(self):
        text = (demo / "stack.ex").read_text()
        model, code = text.split("defv step", 1)
        self.assertIn("[a - b | t]", code)
        report = self.reject("stack", model + "defv step" + code.replace("[a - b | t]", "[b - a | t]"),
                             "env", "list")
        self.unproved(report, ":stack.exec/3")

    def test_compiler(self):
        for engine in ["z3", "boogie"]:
            self.verify("env", "list", "expr", "stack", "compile", engine=engine)

    def test_compiler_runtime(self):
        values = self.runtime(["env", "list", "expr", "rewrite", "stack", "compile"], '''
        leaf = [{:const, 0}, {:const, -7}, {:var, 0}, {:var, 4}]
        trees = for op <- [:add, :sub], a <- leaf, b <- leaf, do: {op, a, b}
        deep = Enum.reduce(1..20, {:var, 0}, fn i, e -> {:sub, {:const, i}, e} end)
        results = for e <- [deep | trees], ctx <- [{}, {9}, {-3, 4}], s <- [[], [5], [2, 8]] do
          expected = [:expr.eval(e, ctx) | s]
          :stack.exec(:compiler.run(e), s, ctx) === expected and
            :stack.exec(:compiler.run(:rewrite.run(e)), s, ctx) === expected
        end
        order = :compiler.run({:sub, {:const, 10}, {:const, 3}}) ===
                  [:sub, {:const, 3}, {:const, 10}]
        IO.puts(JSON.encode!([order | results]))
        ''')
        self.assertEqual(values, [True] * 298)

    def test_compiler_rejects_reversed_code_order(self):
        text = (demo / "compile.ex").read_text()
        model, theorem = text.split("defvg correct", 1)
        self.assertIn(":seq.cat(code(elem(e, 2)), code(elem(e, 1)))", model)
        model = model.replace(":seq.cat(code(elem(e, 2)), code(elem(e, 1)))",
                              ":seq.cat(code(elem(e, 1)), code(elem(e, 2)))")
        model = model.replace(":stack.join(y, x)", ":stack.join(x, y)")
        model = model.replace(":seq.append(run(b), run(a))", ":seq.append(run(a), run(b))")
        report = self.reject("compile", model + "defvg correct" + theorem,
                             "env", "list", "expr", "stack")
        status = {c["owner"]: c["status"] for c in report["contracts"]}
        self.assertEqual(status[":compiler.run/1"], "proved")
        self.assertEqual(status[":compiler.correct/3"], "unproved")

    def test_parser(self):
        for engine in ["z3", "boogie"]:
            self.verify("parse", engine=engine)
        inputs = [list(xs) for n in range(8) for xs in itertools.product([40, 41, 120], repeat=n)]
        inputs += [[40] * 128 + [41] * 128, [40] * 128 + [41] * 127, [40, 41] * 4]
        expected = [len(xs) // 2 if xs == [40] * (len(xs) // 2) + [41] * (len(xs) // 2)
                    else "error" for xs in inputs]
        body = 'xs = ' + json.dumps(inputs) + '\n'
        body += 'IO.puts(JSON.encode!(Enum.map(xs, fn xs -> :parse.run(List.to_tuple(xs)) end)))\n'
        self.assertEqual(self.runtime(["parse"], body), expected)

    def test_parser_rejects_missing_progress(self):
        report = self.mutate("parse", "{k, n} = walk(xs, j)", "{k, n} = walk(xs, i)")
        self.assertTrue(any(e["status"] != "proved" and ": decrease" in e["detail"]
                            for e in report["evidence"]))

    def test_parser_rejects_wrong_count(self):
        report = self.mutate("parse", "{last, n + 1}", "{last, n + 2}")
        self.unproved(report, ":parse.run/1")
