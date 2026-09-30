# components

This document describes each component of vex: what it does, which files implement
it, and how it communicates with the other components. It was written from the
working tree on 2026-09-30 (source protocol `vex.ir.2`, semantic profile
`erlang.discrete.2`). The tree was being edited at the time, and `flow` was being split
into smaller modules. If a file named here has moved, search for the function name.

Related documents: [design](design.md) (why), [protocol](protocol.md) (stage
interfaces), [semantics](semantics.md) (meaning), [trust](trust.md) (what a result
claims), [proofs](proofs.md) (Lean workflow), [extend](extend.md) (how to grow it),
[coverage](coverage.md) (supported features).

## at a glance

| component | path | language | runs as | responsibility |
| --- | --- | --- | --- | --- |
| front | `front/` | Elixir | child process `elixir front/read.exs`; also a Mix package used by applications | read Elixir source without executing it and emit the `vex.ir.2` JSON document; runtime macros that erase proof code |
| kernel | `kernel/src/` | Rust library | linked into `vex` | IR schema, sorted logic, Erlang operation semantics, shared datatype description, verification condition type, stable ids, name encoding |
| kernel lean library | `kernel/lean/` | Lean 4 | built by `lake`, imported by every generated Lean obligation | generated term theory, lemmas, automation tactic, axiom audit, Hoare and actor theorems |
| flow | `flow/src/` | Rust library | linked into `vex` | validate the program graph, execute symbolically, generate verification conditions and TLA+ source exports |
| back | `back/src/` | Rust library | linked into `vex` | print conditions as SMT-LIB, Boogie and Lean; run external tools; classify their output; generate `vex.tla` and run TLC |
| cli | `cli/` | Rust binary `vex` | the entry point | arguments, configuration, discovery, orchestration, parallel jobs, report, exit codes |
| external tools | `PATH`, `.tools/` | Z3, Boogie (.NET), Lean/lake, Java + TLC, Elixir | child processes | proof search, proof checking and model checking |
| scripts | `bin/`, `Makefile` | sh, make | the user or CI | setup, launcher, validation, step-by-step runs |
| tests | `tests/`, `*/tests/`, `front/test/` | Python, Rust, ExUnit | `bin/check.sh` and CI | regression, negative cases, OTP conformance |

### crate dependencies

```text
                 cli  (binary "vex")
                /    \
             flow    back
                \    /
                kernel
```

`flow` and `back` never depend on each other. `cli` is the only place where source
lowering and tool execution meet. `kernel` has no file discovery, process launching,
solver syntax or CLI. `front` is not a crate: it reaches `kernel` only through a
JSON file.

### data flow of `vex check`

```text
 .ex files ─discover─► paths.json ─elixir front/read.exs─► source.json
                                                              │  serde → kernel::ir::source
                                                              ▼
                                         flow::lower ─► plan.json  (contracts + conditions)
                                                              │  one kernel::vc::vc per obligation
                                                              ▼
                              back::print ─► vc/<id>.smt2 | vc/<id>.bpl | vc/<id>.lean
                                                              │  back::run::command (deadline, process group)
                                                              ▼
                  z3 -smt2 | boogie | lake env lean ─► stdout + exit code  (vc/<id>.<engine>.log)
                                                              │  back::solver::classify
                                                              ▼
                                               evidence { id, engine, status, … }
                                                              │  cli::report::contracts (dependency propagation)
                                                              ▼
                       [[models]] ─flow::model::export─► vex.tla ─TLC─► model evidence
                                                              ▼
                                   report.json + text/JSON on stdout + exit code 0 / 1 / 2
```

## front: the Elixir side

The frontend has two jobs that must agree:

1. Tell the checker what the source means.
2. Compile the same source into an application with every proof statement erased.

### `front/lib/vex.ex`: the runtime macros (module `:vex`)

Applications add `{:vex, path: ".../front"}` and write `use :vex`.

- `use :vex` imports the macros and registers `@verifier` as an accumulating module
  attribute.
- `requires`, `ensures` and `decreases` expand to escaped data, so
  `@verifier requires …` is an inert attribute at compile time.
- `defv` becomes `def` and `defvp` becomes `defp`. Their bodies pass through
  `erase/1`, which removes `ghost`, `assert`, `unfold`, `assume`, `havoc` and `block`.
- `defvg` defines nothing at runtime, because ghost functions exist only for the checker.
- `forall` and `exists` compile to `nil`. `term_size` raises at runtime, because it is
  a proof-only operation.

The ExUnit test `front/test/vex_test.exs` compiles a module whose `ghost` block
raises. It checks that the function still returns the correct value and that the
ghost function is not exported.

### `front/lib/read.ex`: the reader (module `:vex_read`)

`files(paths)` parses each file with `Code.string_to_quoted!`, which only parses. It
never evaluates code or expands macros. For each file it records a SHA-256 hash and
walks the top-level `defmodule`s:

- **Module identity.** `Foo.Bar` becomes `"Foo.Bar"`, `:counter` becomes
  `":counter"`, and `Elixir.Foo` aliases to `"Foo"`. A duplicate module is rejected.
- **Allowed directives.** A module that contains any vex item may only use `use :vex`,
  definitions, typespec and documentation attributes, and attributes with literal
  values. It may not use `alias`, `import`, `require`, compile hooks (`@before_compile`,
  `@on_load`, …) or evaluated attributes. The reader does not expand macros, so any of
  these could make the compiled code differ from what was read.
- **Contracts.** `@verifier requires | ensures | decreases` entries accumulate until
  the next function head and attach to that clause. At most one `decreases` is
  allowed per clause.
- **Selection.** A definition with contracts, or any `defv*`, becomes a `fun` record.
  Every other `def`/`defp` goes to `skipped`, which is the coverage inventory that
  `--strict` enforces.
- **Heads.** A head has a name, argument patterns and at most one `when` guard.
  Formal arguments are renamed `$arg0 … $argN`, so clauses with different patterns
  share one signature. The patterns live in each clause.
- **Clauses.** Clauses with the same name and arity are grouped in source order and
  must have the same visibility and ghost mode. `complete/1` rejects a selected
  function if one of its clauses was left unannotated. Otherwise a runtime clause
  would silently escape verification.
- **Expressions.** Each expression becomes a tagged JSON node: `int`, `atom`, `var`,
  `op`, `call`, `branch` (`if`), `case`, `block`, `bind` (`=`), `tuple`, `list`,
  `assert`, `ghost`, `unfold`, `quant` (`forall`/`exists`), `assume`, `havoc` and
  `local` (`block do … end`).
  - Integers are decimal strings, so arbitrary size survives JSON.
  - Operators in `@ops` become `op`, including the `Kernel.x` and `:erlang.x`
    spellings.
  - Anything else raises `CompileError` with the file and line.

### `front/read.exs`, `front/oracle.exs`, `front/mix.exs`

- `read.exs` is the command the CLI spawns. It runs
  `elixir front/read.exs --manifest paths.json --out source.json`, or takes paths as
  arguments and writes JSON to stdout. On any error it prints the message to stderr
  and exits with code 2.
- `oracle.exs` evaluates `div`, `rem`, `+`, `-` and `*` on the real BEAM. Only the
  conformance test uses it.
- `mix.exs` makes `front/` the Mix package `:vex`.

### front communication

| direction | mechanism | payload |
| --- | --- | --- |
| cli → front | child process: `elixir <root>/front/read.exs --manifest <run>/paths.json --out <run>/source.json`. The working directory is `VEX_ROOT`, `ERL_FLAGS` defaults to `+S 2:2`, and the deadline is `max(timeout_ms, 30 s)` | a JSON array of absolute `.ex`/`.exs` paths |
| front → kernel | file `<run>/source.json` | a `vex.ir.2` document, deserialized into `kernel::ir::source` |
| front → cli | exit code and stderr, saved to `<run>/front.log` | a non-zero exit becomes `vex` exit 2 (`Elixir frontend: …`) |
| application → front | Elixir compile-time macros | unrelated to checking; the verifier never compiles your project |

## kernel: the shared meaning

`kernel` is the only place that says what Erlang values and operations mean. It does
so in a vocabulary that is independent of any solver. It performs no I/O.

### `lib.rs`

- Constants: `version = "vex.ir.2"`, `profile = "erlang.discrete.2"` and
  `lean = "leanprover/lean4:v4.28.0"`.
- `hash` returns SHA-256 as hex.
- `name(raw)` is an injective encoding that prefixes `v` to the hex bytes of the name.
  For example `n` becomes `v6e`, `$arg0` becomes `v2461726730` and `Basic.inc/1`
  becomes `v42617369632e696e632f31`. Every printer uses it, so user text never becomes
  solver or Lean syntax. `test_source_paths_cannot_inject_solver_commands` covers this.

### `ir.rs`: the frontend schema

- `source { version, files[{path, hash}], functions[fun], skipped[skip] }`.
- `fun { module, name, args, mode: public | private | ghost, clauses[clause], span }`,
  with `fun::id() = "Module.name/arity"`.
- `clause { patterns, requires, ensures, guard, decreases?, body, span }`.
- `node { line, kind }`, where `kind` is the tagged union listed under front.

Every record uses `deny_unknown_fields`, so a mismatch between frontend and kernel
fails loudly instead of losing data.

### `logic.rs`: the sorted logical algebra

- **Sorts.** `term` (an Erlang value), `seq` (the field sequence of a tuple), `int` and
  `bool`.
- **`expr`.** One of `integer`, `boolean`, `var`, `app`, `prim`, `ite` or `quant`.
  `app` is an uninterpreted function application; calls to user functions become `app`.
- **`op`.**
  - Constructors: `int atom nil cons tuple empty push`.
  - Testers: `is_*`.
  - Selectors: `ival aval head tail items first rest`.
  - Recursive functions: `len nth size mass`.
  - Logic: `eq not and or`.
  - Arithmetic: `add sub mul div lt le`.
- **Smart constructors simplify eagerly.** This covers constant folding, equality of
  constructors, a selector applied to a constructor, a tester applied to a
  constructor, boolean identities and `ite` collapse. Many trivial obligations
  disappear before any solver runs.
- **Boolean bridges.** `bool_term`, `true_term`, `truthy` and `is_bool` map between
  Erlang boolean atoms and logical booleans. `false` is atom 0, `true` is atom 1 and
  `nil` is atom 2.
- **Helpers.** `replace` performs capture-avoiding substitution. `symbols` collects
  free variables and function symbols so printers can declare them.

### `theory.rs`: one datatype description for every backend

- **`terms`.** `int(ival: Int)`, `atom(aval: Int)`, `nil`, `cons(head, tail)` and
  `tuple(items: seq)`.
- **`sequences`.** `empty` and `push(first, rest)`.
- **`definitions()`.** Recursive equations for `len`, `nth`, `size` (structural term
  size) and `mass` (the total size of a sequence).
- **`lemmas(vc)`.** Quantified helper facts, selected per obligation: `len ≥ 0`,
  `len = 0 ⇔ empty`, `size ≥ 1`, `mass ≥ 0`, the size bound of `nth`, and sequence
  extensionality. Only lemmas whose operations occur in the obligation are included.
  They are proved in `kernel/lean/seq.lean`, and the SMT and Boogie printers assert
  them as axioms.

### `bif.rs`: Erlang operations

`apply(name, args) -> spec { value, safe }` expresses each supported operation as a
value plus a safety predicate. A runtime error becomes an obligation that
`safe` holds.

| operations | value | safe when |
| --- | --- | --- |
| `+ - *` | integer arithmetic on `ival` | both operands are integers |
| `div rem` | truncating division via `trunc`; `rem = a - div*b` | both are integers and the divisor is not 0 |
| `< <= > >=` | integer comparison | both are integers (no cross-type ordering) |
| `=== == !== !=` | constructor equality | always |
| `is_integer is_atom is_tuple is_list is_boolean is_nil` | boolean atom | always |
| `hd tl` | `head` / `tail` | the argument is a cons |
| `elem(t, i)` | `nth(items t, ival i)` | `t` is a tuple, `i` is an integer and `0 ≤ i < len` |
| `tuple_size` | `len(items t)` | `t` is a tuple |
| `not` / `!` | strict negation / truthiness negation | `not` needs a boolean; `!` is always safe |
| `term_size` | structural size | always (proof code only) |

Any other name fails with `unsupported operation … in erlang.discrete.2`.

### `vc.rs`: a verification condition

`vc { id, owner, kind, span, hypotheses, goal, deps }` means: for all free variables,
the conjunction of `hypotheses` implies `goal`. The `id` is the first 24 hex
characters of SHA-256 over `(owner, kind, hypotheses, goal, deps, profile)`. The span
is deliberately excluded. Moving a file or adding unrelated lines therefore keeps
the same ids and Lean proof files (`kernel/tests/vc.rs`), while any semantic change
produces a new id.

### `model.rs`

`export { args, pre, value }` is the interface between `flow::model`, which computes
the meaning of a pure function, and `back::tla`, which prints it for TLA+.

## kernel lean library

This is a Lean package (`lakefile.toml`: library `kernel`, `-DautoImplicit=false`),
pinned by `lean-toolchain` to `leanprover/lean4:v4.28.0`, with no Mathlib.

| file | role |
| --- | --- |
| `term.lean` | **Generated** by `vex kernel` from `back::print::lean::theory()`. It holds the mutual inductives `term`/`seq`, the testers as `Prop`, selectors with defaults, and recursive `len nth size mass`. `back/tests/solver.rs` asserts that the committed file equals the generator output |
| `seq.lean` | Proofs of the lemmas that `theory::lemmas` hands to SMT and Boogie as axioms (`len_nonneg`, `len_zero`, `size_pos`, `mass_nonneg`, …) |
| `audit.lean` | `#vex_audit name` collects the axioms of a declaration and rejects everything except `propext`, `Classical.choice` and `Quot.sound`. It prints `vex audit ok: name`, which the classifier requires |
| `auto.lean` | The `vex_auto` tactic: split conjunctive hypotheses, `subst_vars`, then `assumption`, `And.intro`, `omega`, or `simp_all` with size/length bounds (`vex_bounds`) and case splits |
| `logic.lean` | `hoare`/`total` definitions, `strengthen`, `compose`, `integer_injective`, and the Erlang `trunc`/`rem` division identity |
| `actor.lean` | Reachability, the invariant induction rule, and a simulation (refinement) theorem. These are foundations for future actor refinement. Generated obligations do not use them |

Every generated `vc/<id>.lean` starts with `import kernel.lean.auto`. Before the first
Lean obligation of a run, `back::solver` builds this library once with
`lake +leanprover/lean4:v4.28.0 build kernel`, so stale `.olean` files are never used.

## flow: from source to obligations

`flow` turns `kernel::ir::source` into a `plan`. It knows the meaning of Elixir
(through `kernel`) and nothing about solver syntax.

Entry point: `flow::lower(&source) -> plan { profile, atoms, contracts, conditions }`.

### `graph.rs`: the validated program (`world`)

- **Checks.** It verifies:
  - the protocol version;
  - that the selection is not empty;
  - that there are no duplicate ids;
  - that arguments are distinct;
  - that each clause has the right arity;
  - that bindings are in statement position (via `scope.rs`).
- **Atom interning.** `false`=0, `true`=1 and `nil`=2 are fixed. Other atoms are
  sorted and numbered from 3. The table is recorded in `plan.atoms`.
- **Call edges.** An unqualified call resolves to the caller's own module. A call to
  anything without a contract is an error:
  `… calls X, which has no supported contract`.
- **Queries.**
  - `reaches`, `recursive` and `same_cycle`.
  - `rank(f, clause)` returns the clause's `decreases` expression. A tuple is a
    lexicographic measure. A recursive ghost with one argument defaults to that
    argument.
  - `total(f)` holds when every recursive function reachable from `f` has a measure
    on every clause. A recursive ghost without a measure is rejected, because an
    unmeasured ghost equation could be inconsistent.

### `scope.rs`, `pattern.rs`

- `scope.rs` rejects bindings in expression position, such as `f(x = 1)`. Elixir's
  sibling-scope rule would need a separate lowering.
- `pattern.rs` provides `matches(pattern, value, env)`. It returns a logical test and
  extends the environment:
  - variables bind, and a repeated variable adds an equality;
  - `_` matches anything;
  - integer and atom literals test equality, and negative integer literals are
    supported;
  - tuples test `is_tuple`, then walk `items` with `push`/`first`/`rest`, which
    enforces the exact length;
  - lists walk `cons`/`head`/`tail`, with an optional tail;
  - `p = q` must match both.

### `spec.rs`: contract expressions

This file evaluates `requires`, `ensures`, guards and measures to
`value { term, safe, deps }`.

- **Calls inside a specification.**
  - A call to the function itself denotes the current return value. If its arguments
    are not syntactically the original ones, their equality to the originals becomes
    part of the expression's safety condition.
  - Any other call must name a ghost function whose precondition holds.
  - Calls to other executable functions are rejected.
- **Quantifiers.** `forall` and `exists` bind term-sorted variables. The body must be
  well-defined and boolean for every instance.
- **`operation`.** Implements strict `and`/`or`, whose left operand must be boolean,
  and truthy `&&`/`||`, both with short-circuit semantics. Everything else goes
  through `bif::apply`.
- **`pre(f, args)`.** The disjunction, over the clauses that could be selected at
  runtime, of that clause's selection test together with its `requires`.

### `clause.rs`: runtime dispatch

- `select(f, args)` gives each clause a selection test. The test holds when the
  clause's patterns and guard match and no earlier clause matched, which is Erlang's
  first-match order. `requires` never influences selection.
- `guard_syntax` limits guards to guard-safe forms: no `&&`, `||`, `!`, calls or
  `term_size`.
- `post(f, args, result)` is the conjunction, over clauses, of "this clause selected
  ⇒ its `ensures` hold of `result`".

### `exec.rs`: the symbolic executor

`check(world, f)` runs for each clause whose selection test is not `false`:

1. The initial facts are `[selection test]`.
2. Each `requires` emits a `domain` obligation (it must be well-defined and boolean),
   then becomes a fact.
3. A measure, if any, emits a `measure` obligation (its components are ≥ 0).
4. The body is evaluated over symbolic **paths**. A path is a state plus a value. The
   state holds the variable environment, facts (the hypotheses of later
   obligations), dependencies, the measure, and flags for proof mode and unfolding.
5. Every final path emits a `post` obligation for that clause's `ensures`.

The main rules:

- **Operations.** Emit `safety` with `bif`'s safe predicate. The predicate then becomes
  a fact.
- **Calls.** Emit `call` for the callee's precondition. If the callee is in the
  caller's measured recursive cycle, also emit `decrease`, a lexicographic
  "less than". The result is the uninterpreted application `Module.f/n(args)`, and
  the callee's **postcondition** is added as a fact. Callee bodies are never inlined,
  so reasoning is modular.
  - Executable code cannot call a ghost.
  - Proof code may only call total functions.
  - A private function cannot be called from another module.
- **`=`.** Emits `match`.
- **`if`.** Produces two paths, guarded by truthy and falsy facts.
- **`case`.** Arms are tried in order, each excluding the earlier arms, and a
  `coverage` obligation requires that some arm matches. Variables bound inside a
  branch do not leak out.
- **`and`/`or`.** Emit `boolean` for the left operand, then split into short-circuit
  paths.
- **Resource limits.** More than 1024 paths, depth 128, 10 000 obligations or
  specification nesting 96 produce an error that asks you to split the function into
  contracts.

### `proof.rs`: erased proof statements

`quantified` evaluates a quantified ghost value through `flow::spec`, checks
its boolean domain and definedness, and records dependencies. It rejects runtime
quantifiers, partial calls and calls within the enclosing recursive component,
where specification evaluation would otherwise bypass decrease checking.

| statement | rule |
| --- | --- |
| `assert e` | `assert` obligation, then a fact |
| `ghost do … end` | Evaluated in proof mode. Variables are restored afterwards and the value is erased |
| `unfold g(x)` | Checks the call like any other call. Then it instantiates the body of the **total** function `g` (normally a ghost) locally, adding `g(x) == body` as a fact. Forbidden within the owner's own recursive component |
| `assume e` | Ghost code only. Emits an `assumption_domain` obligation, then adds `e` as a fact **and records an admission**. The report marks the contract, and every contract that depends on it, as `conditional` |
| `havoc x` | Ghost code only. Replaces `x` with a fresh variable |
| `block do … end` | Ghost code only. Inner obligations are checked, then the facts are discarded |

### `rank.rs`, `model.rs`, `lib.rs`

- `rank.rs` evaluates a measure component. An integer component contributes its value;
  any other term contributes its structural `size`. Every component must be ≥ 0.
- `model.rs` provides `export(world, id)` for TLA+. The function must be executable and
  nonrecursive. Arguments are mathematical integers wrapped as `int(v)`. It computes
  `pre` (the precondition plus safety) and `value` by inlining the body: blocks,
  binds, `if` and nonrecursive calls. `case` is not supported yet. A nested call must
  first be bound to a variable.
- `lib.rs` contains `lower` and the types `contract`, `admission` and `plan`. After
  generating the conditions, it propagates **totality** to a fixpoint: a function that
  depends on a partial function is itself partial.

### obligation kinds

| kind | emitted when | goal |
| --- | --- | --- |
| `domain` | each `requires` | the expression is well-defined and boolean |
| `measure` | a clause has a measure | the measure is well-defined and ≥ 0 |
| `safety` | each runtime operation | `bif` safe predicate (types, divisor, bounds) |
| `boolean` | strict `and`/`or` | the left operand is `true` or `false` |
| `match` | `pattern = value` | the pattern matches |
| `coverage` | `case` | some arm matches |
| `call` | each call | the callee's precondition for the selected clause |
| `decrease` | a call inside a measured recursive cycle | the lexicographic measure decreases |
| `assert` | `assert` | the asserted expression holds |
| `assumption_domain` | `assume` | the assumed expression is well-defined |
| `post` | each final path | the clause's `ensures` hold |

## back: printers and tool execution

`back` owns all target syntax and every external process. It never sees Elixir AST.
Its input is `kernel::vc::vc` or `kernel::model::export`, and its output is
`evidence`.

### `print/`

- **`expr.rs`** renders one `kernel::logic::expr` tree in three dialects: `smt`, `bpl`
  and `lean`. Datatype operations are named `k_<op>` in SMT and Boogie and
  `term.<op>`/`kernel.<op>` in Lean. Variables and functions go through
  `kernel::name`.
- **`smt.rs`**
  - `theory()` declares `term`/`seq` with `declare-datatypes`, defines guarded
    selectors that return a default on the wrong constructor, and defines
    `len nth size mass` with `define-funs-rec`.
  - `emit(vc)` writes the timeout option, the theory and the needed lemmas; declares
    each free variable (`declare-const`) and each called user function
    (`declare-fun f (term …) term`); asserts the hypotheses; asserts
    `(not goal)`; and ends with `(check-sat)`. `unsat` means the implication holds.
- **`bpl.rs`**
  - `theory()` models datatypes as uninterpreted types with a tag function and
    constructor, selector and reconstruction axioms. This deliberately
    overapproximates: non-well-founded values are allowed (see trust).
  - The obligation becomes
    `procedure check(vars) { assume h…; assert goal; }`.
- **`lean.rs`**
  - `theory()` generates `term.lean`.
  - `emit(vc, proof)` produces
    `theorem vc_<id> (vars : term) (funs : term → … → term) (h0 : …) … : goal :=`,
    followed by `by vex_auto` or the user's proof term from `--proofs DIR/<id>.lean`,
    then `#vex_audit vc_<id>`.

### `run.rs`: bounded child processes

`command(program, args, cwd, timeout)` controls every child process:

- The child runs in its own process group, with stdin set to null.
- stdout and stderr are drained on separate threads and capped at 16 MiB each. Output
  over the cap is marked truncated, and the classifier treats that as an error.
- The parent polls every 10 ms. It kills the whole group with SIGKILL on timeout, and
  again after exit, so a solver's helper process cannot keep the pipes open.
- The child environment gets `ERL_FLAGS=+S 2:2` unless it is already set, plus
  `DOTNET_ROOT` when it needs fixing, `DOTNET_ROLL_FORWARD=Major` and the .NET
  telemetry opt-out. The user's shell is never modified.

`save` writes files and creates parent directories. `locate` finds a bare tool name
on `PATH`.

### `solver.rs`: engines, statuses, evidence

`check(vc, engine, settings)` works in these steps:

1. Print the artifact to `vc/<id>.<suffix>`.
2. For Lean, build the Lean library once (`OnceLock`). A failed build is an `error`.
3. Run the tool with a deadline of `timeout_ms + 2 s`, and save
   `vc/<id>.<engine>.log`.
4. Classify the result. On a Z3 `sat`, rerun with `(get-model)` and save
   `<id>.model.smt2` and `<id>.model.log`.

| engine | command (working directory = `VEX_ROOT`) | `proved` requires |
| --- | --- | --- |
| z3 | `z3 -smt2 vc/<id>.smt2` (also `(set-option :timeout …)` inside the file) | exit 0 and output exactly `unsat`. `sat` → counterexample; `unknown` → unknown; anything else → error |
| boogie | `boogie /timeLimit:<s> /errorLimit:1 /proverOpt:PROVER_PATH=<z3> vc/<id>.bpl` | exit 0 and the exact line `… finished with 1 verified, 0 errors`. "might not hold" → counterexample; inconclusive or timeout → unknown. `0 verified` is an error |
| lean | `lake +leanprover/lean4:v4.28.0 env lean vc/<id>.lean` | exit 0, `vex audit ok` present and no `sorry`. Unsolved goals or failed tactics → unknown; parse and elaboration errors → error |

Statuses: `proved`, `model_checked` (TLC only), `counterexample`, `unknown`,
`timeout`, `unavailable` (the tool could not start) and `error`.

An `evidence` record is `{ id, engine, status, artifact, log, detail (≤1200 chars), ms }`.

`emit(vc, engine)` writes the artifact and runs nothing. `vex emit` uses it.

### `tla.rs`: source exports and TLC

- **`module(exports)`** generates `vex.tla`:
  - It extends `Integers` and `Sequences`, and declares the recursive `k_len`,
    `k_nth`, `k_size` and `k_mass`.
  - For each export `name` it defines `name_pre(args)` and `name(args)`.
  - Terms become records such as `[tag |-> "int", ival |-> n]`. That is why a model
    writes `step(balance, delta).ival`.
  - Uninterpreted calls and quantifiers are rejected.
- **`check(file, cfg, exports)`** runs TLC:
  1. Stage the model in `<run>/models/<i>/`: copy every `.tla` from the model's
     directory, copy the configuration as `model.cfg` and write the generated
     `vex.tla`.
  2. Record SHA-256 hashes of all inputs, including the TLC jar, in `inputs.json`.
  3. Run `java -XX:ActiveProcessorCount=2 -Xmx1g -cp tla2tools.jar tlc2.TLC -workers 1 -tool -metadir states -config model.cfg <model>.tla`
     in the stage directory.
  4. Classify the output. `Model checking completed. No error has been found.` with
     exit 0 is `model_checked`. `is violated`, `Deadlock reached` or
     `Temporal properties were violated` is a `counterexample`. Anything else is a
     timeout or an error.
  5. The evidence id is the hash of the inputs.

### back communication

| direction | mechanism | payload |
| --- | --- | --- |
| cli → back | function calls `solver::check`, `solver::emit`, `tla::module`, `tla::check` | `&vc`, engine name, `settings` |
| back → tools | child process through `run::command` | the artifact path as an argument |
| tools → back | stdout, stderr, exit code, and a timed-out flag | classified into `status` |
| back → Lean library | `import kernel.lean.auto` in each file; one `lake build kernel` per run | the theory, the tactic and the audit command |
| back → cli | return values | `evidence` records |

## cli: the `vex` binary

| file | role |
| --- | --- |
| `main.rs` | Parses arguments and calls `app::run`. Any error prints `vex: …` and exits with code 2 |
| `args.rs` | Commands `check emit model doctor kernel help version`. Options `--engine --config --out --proofs --jobs --timeout --json --strict --cfg --jar`. Validates the engine name, jobs 1–128 and a positive timeout |
| `config.rs` | `resolved::new` (details below). `solver()` turns the result into `back::solver::settings` |
| `project.rs` | `discover`, `read`, `unchanged` and `output` (details below) |
| `jobs.rs` | The worker pool and the engine policy (details below) |
| `report.rs` | Contract statuses, the report schema and the human-readable output (details below) |
| `app.rs` | Per-command orchestration (next section). `versions()` asks every tool for its version. `finish` writes the report and chooses the exit code |
| `build.rs` | Hashes the verifier's own sources (`kernel`, `flow`, `back`, `cli`, `kernel/lean`, `front/lib`, `read.exs`, lockfiles and manifests) into `VEX_BUILD`, which the report stores as `build` |

**Configuration (`config.rs`).**

- **Root.** `$VEX_ROOT`, or else the checkout the binary was compiled from.
- **Configuration file.** `./vex.toml` loads automatically when `check`, `emit` or
  `model` receive no paths.
- **Precedence.** Command-line options beat `vex.toml`, which beats the defaults:
  engine `auto`, 2 jobs, 10 000 ms and output directory `./.vex`.
- **Relative paths.** Command-line paths are relative to the current directory.
  Configuration paths are relative to the configuration file.
- **Tools.** Bare names are looked up on `PATH`. Boogie defaults to
  `.tools/boogie/boogie`, and TLC to `<root>/.tools/tla2tools.jar`.

**Source handling (`project.rs`).**

- `discover` expands globs and recurses into directories. It keeps only
  `.ex`/`.exs`, skips `.git .vex .tools target deps _build` and does not follow
  symlinks. An empty selection is an error.
- `read` writes `paths.json`, spawns the frontend and parses `source.json`. It then
  checks that the file inventory and order match and that the hashes still match,
  and enforces `--strict`.
- `unchanged` re-hashes every source at the end of a run.
- `output` creates a fresh `run_<nanoseconds>_<pid>` directory.

**Worker pool (`jobs.rs`).** `jobs` scoped threads pull obligations by an atomic
index. The engine policy:

| engine | runs | passes when |
| --- | --- | --- |
| `z3`, `boogie` or `lean` | that engine only | the result is `proved` |
| `auto` | z3, then boogie, then lean. It stops at `proved`, `counterexample` or `error` and escalates on `unknown`, `timeout` or `unavailable` | the last result is `proved` |
| `all` | all three | all three are `proved` |

**Report (`report.rs`).**

- A contract is `unproved` if any of its own conditions did not pass. This propagates
  to every contract that depends on it, transitively.
- Otherwise it is `conditional` if it, or a dependency, used `assume`.
- Otherwise it is `proved`.
- Correctness is `total` or `partial`, taken from the plan.

`report` is schema `vex.report.1`, with the fields `build`, `schema`, `profile`,
`lean`, `engine`, `scope`, `success`, `artifacts`, `files`, `unchecked`, `tools`,
`contracts`, `evidence` and `models`.

### what each command does

**`vex check`** (`app::run`):

1. `resolved::new`: merge options, `vex.toml` and defaults.
2. `versions()`: run `elixir --version`, `z3 --version`, `boogie /version`,
   `lake +… env lean --version` and `java -version`, and record the results.
3. `project::output`: create `<out>/run_<ns>_<pid>/`.
4. `project::read`: discover, write `paths.json`, run the frontend, load
   `source.json`, verify hashes and apply `--strict`.
5. `flow::lower`: produce the plan, written to `plan.json`.
6. `jobs::run`: call `back::solver::check` for each condition and engine.
7. `report::contracts`: propagate failures and conditionality.
8. `models()`: for each `[[models]]` entry, compute `flow::model::export` for every
   configured export, then `back::tla::module` and `back::tla::check`.
9. `project::unchanged`: fail if any source changed during the run.
10. `success` requires every contract to be `proved` and every model to be
    `model_checked`.
11. `finish`: write `report.json` into the run directory **and** into
    `<out>/report.json`, print, and exit 0 or 1.

**The other commands:**

- **`vex emit`** runs steps 1–5, then writes artifacts for the selected engines. `auto`
  and `all` write all three formats. It exits 0 without proving anything.
- **`vex model file.tla [--cfg file.cfg]`** checks only the TLA+ models. It reads
  sources only when a configured model has exports. Its report scope says that
  contracts were not checked.
- **`vex doctor`** prints tool versions and the TLC jar hash. It exits 0 only if every
  tool is available and Lean is 4.28.0; otherwise it exits 2.
- **`vex kernel`** regenerates `kernel/lean/term.lean` from the Rust datatype
  description.
- **`help`** and **`version`** print text. The version string includes the profile.

### exit codes

| code | meaning |
| --- | --- |
| 0 | every selected contract is `proved` and every model is `model_checked` |
| 1 | an obligation or model failed or is unresolved, or a contract is `conditional` |
| 2 | invalid input or setup: bad options or configuration, unsupported source, frontend failure, an empty selection, a `--strict` violation, a source that changed during the run, or `doctor` finding a problem |

## how everything communicates

vex has no daemon, socket or shared memory. Inside one `vex` process, the crates
exchange plain Rust values defined by `kernel`. Across processes, everything
travels as files, arguments, stdout and exit codes, and each exchange is preserved in
the run directory.

| # | from → to | mechanism | payload / artifact |
| --- | --- | --- | --- |
| 1 | user → cli | argv and `vex.toml` | commands, paths, options, models |
| 2 | cli → front | child process | `paths.json` → `source.json`, `front.log` |
| 3 | front → kernel | JSON file plus serde | `vex.ir.2` document |
| 4 | cli → flow | function call | `&kernel::ir::source` → `flow::plan` (`plan.json`) |
| 5 | flow → kernel | function calls | `logic`, `bif`, `theory` → `kernel::vc::vc` |
| 6 | cli → back | function call per obligation and engine | `vc` + `settings` → `evidence` |
| 7 | back → Z3, Boogie, Lean | child process | `vc/<id>.smt2`, `.bpl`, `.lean` → `vc/<id>.<engine>.log` |
| 8 | back → Lean library | Lean import and one build per run | `lean_build.log` |
| 9 | cli → flow::model → back::tla | function calls | `kernel::model::export` → `vex.tla` |
| 10 | back::tla → TLC | child process | `models/<i>/{*.tla, model.cfg, vex.tla, inputs.json, tlc.log, states/}` |
| 11 | cli → user | stdout, files and exit code | text or `--json` report, `report.json`, 0/1/2 |
| 12 | application → front | Elixir macros at the application's own compile time | the erased runtime code; the checker is not involved |

### run directory layout

```text
.vex/
  report.json                      copy of the latest report
  run_<nanoseconds>_<pid>/
    paths.json                     sorted absolute source paths (frontend input)
    source.json                    vex.ir.2 document (frontend output)
    front.log                      frontend stdout/stderr
    plan.json                      atoms, contracts, conditions
    vc/
      <id>.smt2  <id>.bpl  <id>.lean     one artifact per engine that ran
      <id>.z3.log  <id>.boogie.log  <id>.lean.log
      <id>.model.smt2  <id>.model.log    Z3 counterexample model, when sat
      lean_build.log                     one Lean library build per run
    models/<i>/                    staged TLA+ inputs, inputs.json, tlc.log, states/
    report.json                    this run's report
```

No run reads results from an older run directory. Nothing is cached.

## worked example: `Basic.inc/1` and `Basic.twice/1`

The source is `demo/basic.ex`:

```elixir
@verifier requires is_integer(n)
@verifier ensures inc(n) === n + 1
defv inc(n), do: n + 1

@verifier requires is_integer(n)
@verifier ensures twice(n) === n + 2
defv twice(n) do
  x = inc(n)
  inc(x)
end
```

**Frontend.** `source.json` holds one `fun` per function, with `args: ["$arg0"]` and a
single clause. The clause's `patterns` is `[var n]`, `requires` is
`[op is_integer(var n)]`, `ensures` is `[op ===(call inc(n), op +(n, 1))]`, `guard`
is the atom `true`, and `body` is `op +(n, 1)`.

**flow.** `inc` produces two conditions:

- `safety` for `n + 1`: given the selection test (`True`) and the precondition fact
  `is_int n`, prove `is_int n`.
- `post`: `inc(n)` in `ensures` denotes the result `int(ival n + 1)`. The simplifier
  folds `int(ival n + 1) = int(ival n + 1)` to `true`, so the remaining goal is the
  well-definedness of `n + 1` inside `ensures`: `is_int n`.

`twice` produces two `call` conditions, one for each `inc` precondition, and one
`post`. Its plan entry records `deps: ["Basic.inc/1"]`.

**Printed obligations** (from `vex emit demo/basic.ex`). `$arg0` is encoded as
`v2461726730` and `Basic.inc/1` as `v42617369632e696e632f31`:

```smt2
; 77fb0cf81b9b29edeb47cfe8 safety …/demo/basic.ex:6
(declare-const v2461726730 term)
(assert true)
(assert (k_is_int v2461726730))
(assert (not (k_is_int v2461726730)))
(check-sat)
```

```boogie
procedure check(v2461726730:term)
{
  assume true;
  assume k_is_int(v2461726730);
  assert k_is_int(v2461726730);
}
```

Modular reasoning is visible in the Lean statement of `twice`'s postcondition. `inc`
is an arbitrary function parameter. Hypotheses `h3` and `h5` are `inc`'s
postcondition, instantiated at each call:

```lean
theorem vc_1ed881fe409647cf5bcea7e9 (v2461726730 : term) (v42617369632e696e632f31 : term → term)
    (h0 : True)
    (h1 : (kernel.is_int v2461726730))
    (h2 : (kernel.is_int v2461726730))
    (h3 : ((kernel.is_int v2461726730) ∧ ((v42617369632e696e632f31 v2461726730) = (term.int ((kernel.ival v2461726730) + (1 : Int))))))
    (h4 : (kernel.is_int (v42617369632e696e632f31 v2461726730)))
    (h5 : ((kernel.is_int (v42617369632e696e632f31 v2461726730)) ∧ ((v42617369632e696e632f31 (v42617369632e696e632f31 v2461726730)) = (term.int ((kernel.ival (v42617369632e696e632f31 v2461726730)) + (1 : Int)))))) :
    ((kernel.is_int v2461726730) ∧ ((v42617369632e696e632f31 (v42617369632e696e632f31 v2461726730)) = (term.int ((kernel.ival v2461726730) + (2 : Int))))) :=
by
  vex_auto

#vex_audit vc_1ed881fe409647cf5bcea7e9
```

**Evidence.** Z3 prints `unsat`, Boogie prints
`Boogie program verifier finished with 1 verified, 0 errors`, and Lean prints
`vex audit ok: vc_…`. With `--engine all`, each condition has three `proved`
evidence records. `report.json` then lists `Basic.inc/1` and `Basic.twice/1` as
`proved`, with correctness `total`. If `inc`'s postcondition had failed, `twice`
would be reported `unproved`, even though its own obligations pass.

## concurrency: how TLA+ fits

No component extracts actors from Elixir. Concurrency enters only through a TLA+
model that you write, with its own processes, mailboxes, atomicity and fairness:

1. **`vex.toml`.** A `[[models]]` entry with `[models.exports] step = "Counter.step/2"`
   asks `cli` for a source export.
2. **`flow::model::export`.** Lowers the pure function `Counter.step/2` to a `pre` and
   a `value` over integer arguments.
3. **`back::tla::module`.** Prints `step_pre(a, b)` and `step(a, b)` into the
   generated `vex.tla`.
4. **The model.** It says `EXTENDS vex` and uses `step_pre(...)` as an action guard
   and `step(...).ival` as the next state (see `demo/counter/counter.tla`).
5. **`back::tla::check`.** Runs TLC on the finite configuration.

This result links the decision logic to the source. It does not prove that a real
OTP process refines the model. `kernel/lean/actor.lean` holds the theorems a future
refinement step would use.

## scripts, CI and tests

| item | role |
| --- | --- |
| `bin/vex` | Builds the `vex` crate quietly, sets `VEX_ROOT`, then runs `target/debug/vex` |
| `bin/setup.sh` | Checks prerequisites, installs Boogie 3.5.6 into `.tools/boogie`, downloads TLC 1.7.4 and checks its SHA-256, installs Lean 4.28.0 through elan, builds Rust and the Lean library, and runs `vex doctor` |
| `bin/check.sh` | `cargo fmt --check`, `clippy -D warnings`, `cargo test`, `lake build`, `mix format --check-formatted`, `mix test`, `vex doctor`, the Python suites, then `vex check --config vex.toml` |
| `Makefile` | Unnumbered commands listed in run order by `make help`. `check` accepts source paths and backend settings; `check-dafny` selects a translation with its dependencies; `test-dafny` runs focused regressions. Group recipes run sequentially even with `make -j`. Expected-failure targets assert exit code 1 |
| `.github/workflows/check.yml` | Ubuntu CI: installs OTP/Elixir, .NET 8, Java 21, Z3, elan and Rust 1.93, then runs `bin/setup.sh` and `bin/check.sh` |
| `vex.toml` | The repository's own example project: three sources, engine z3, and the counter and mailbox models |
| `kernel/tests/vc.rs` | Proof ids survive relocation but change with the obligation |
| `back/tests/solver.rs` | Strict output classification; `term.lean` matches the generator; timeouts kill descendant processes |
| `front/test/vex_test.exs` | Runtime erasure; the parser never executes source; large integer literals survive |
| `tests/test_cli.py` | End-to-end CLI cases on real tools: all backends, false postconditions, injection, failed callees, safety and coverage, short-circuiting, ghosts and unfolding, termination, strict mode, missing engines, TLC positive and negative runs with exports, Lean `sorry` and axioms, custom proofs, the Lean version pin |
| `tests/test_papers.py` | Multi-clause dispatch, quantifiers, `assume`/`havoc`/`block`, tuple bounds, lexicographic measures (`demo/paper.ex`) |
| `tests/test_dafny.py` | Maximum, concatenation, environments, recursive interpretation and rewriting, stack execution, expression compiler correctness, and parsing. Real Z3/Boogie proofs, runtime oracles, and rejected mutations; ghost quantifier safety and recursion guards |
| `tests/test_conformance.py` | 45 integer cases, including signed and very large values, evaluated on OTP (`front/oracle.exs`) and then proved through Z3, Boogie and Lean |
| `tests/case.py` | Shared harness: a temporary directory, `vex … --out … --json`, and exit-code assertions |

## where to change what

| change | component |
| --- | --- |
| a new Erlang operation | `kernel/src/bif.rs` (meaning), `front/lib/read.ex` (syntax), conformance and negative tests |
| a new value constructor or recursive function | `kernel/src/theory.rs`, then `vex kernel` to regenerate `term.lean`, lemmas in `kernel/lean/seq.lean` |
| a new source construct | `kernel/src/ir.rs` (shape), `front/lib/read.ex` (reader), `flow` (scope, execution rule, obligation kinds), `front/lib/vex.ex` if it must be erased |
| a new backend | `back/src/print/` plus the engine table in `back/src/solver.rs`; it consumes `kernel::vc::vc` only |
| a new report field or policy | `cli/src/report.rs`, `cli/src/jobs.rs` |

See [extend](extend.md) for the rules each change must follow.
