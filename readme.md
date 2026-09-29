# vex

Elixir contracts, a shared Erlang `kernel`, Z3, Boogie, Lean, and TLA+/TLC in one
verification pipeline. The tool is implemented in Rust and Elixir. Every Lean
invocation uses **`leanprover/lean4:v4.28.0`**.

This is an experimental verifier with an explicit supported language profile.
It checks selected contracts and configured temporal models. It does not yet
verify arbitrary Elixir/OTP applications or automatically extract actor behavior.
Unsupported constructs fail with a diagnostic. Unannotated functions appear in
the coverage report; `--strict` makes them an error.

## run it

Prerequisites: Rust 1.85+, Elixir 1.18+, Z3, .NET 8+ SDK, Java 11+, Elan, Python 3,
curl and ripgrep. The setup installs Boogie **3.5.6** locally, checks the hash of
TLC **1.7.4**, and builds Lean **4.28.0**. Solver versions are retained in reports.

```sh
bin/setup.sh
bin/vex check demo/fib.ex
bin/vex check /path/to/your/app/lib --strict
bin/vex check demo/basic.ex --engine all
bin/vex check --config demo/counter/vex.toml
bin/vex model demo/mailbox/mailbox.tla
```

`bin/vex` builds the Rust CLI and preserves your current directory when resolving
input paths. For repeated runs, use `target/debug/vex` directly. A moved binary
can find its companion files through `VEX_ROOT=/path/to/this/repository`.

To compile the annotations in your application, add the local Elixir package:

```elixir
{:vex, path: "/path/to/z3-erl/front"}
```

```elixir
defmodule Counter do
  use :vex

  @verifier requires is_integer(state) and is_integer(delta) and state >= 0
  @verifier ensures step(state, delta) >= 0
  defv step(state, delta) do
    next = state + delta
    if next < 0, do: 0, else: next
  end
end
```

`defv` is public, `defvp` is private, and `defvg` is mathematical ghost code.
Contracts can also precede ordinary `def`/`defp`. In a postcondition,
`step(state, delta)` denotes this invocation's return value. Other specification
calls refer to verified ghost functions. `ghost`, `assert`, and `unfold` have no
runtime effects. The checker parses quoted source without running application
code or expanding application macros.

The [Fibonacci example](demo/fib.ex) demonstrates ghost definitions,
accumulator invariants, unfolding and checked termination. A recursive ghost
function must have a decreasing nonnegative integer measure. A single argument
provides the default; use `@verifier decreases n - i` for other cases. Executable
recursion without a checked measure is reported as **partial correctness**.

## choose an engine

| option | result |
| --- | --- |
| `--engine z3` | direct SMT obligations checked by Z3 |
| `--engine boogie` | real Boogie programs checked by Boogie, using Z3 |
| `--engine lean` | the same obligations checked as Lean theorems, with axiom audit |
| `--engine auto` | Z3 first, then Boogie and Lean for unknown, timed out, or unavailable results |
| `--engine all` | every obligation must pass all three routes |

`auto` is the default without configuration. Counterexamples and malformed tool
responses stop fallback. Boogie and direct Z3 share a solver, so agreement is not
independent proof certification. Lean constructs a separate proof term.

```sh
bin/vex emit demo/basic.ex --engine lean
bin/vex check demo/basic.ex --engine lean --proofs my_proofs
```

The output contains `plan.json`, `source.json`, and one file per condition. Put a
Lean **proof term**, starting with `by`, in `my_proofs/<condition-id>.lean` to
replace the automatic tactic for that obligation. The theorem statement remains
generated. Inspect its binders in the emitted file. Condition identities include
the proposition, owner, obligation kind, dependencies and semantic profile. Changed
obligations do not silently reuse old proofs. `sorry`, extra axioms and native
decision trust extensions fail the audit. See [proofs](docs/proofs.md).

Source spans remain in the report but do not affect proof identities. Moving a
checkout or adding unrelated lines preserves applicable proof files.

## project configuration

```toml
sources = ["lib/**/*.ex"]
engine = "auto"
jobs = 4
timeout_ms = 10000
out = ".vex"
proofs = "proofs"
strict = true

[tools]
z3 = "z3"
# boogie = "/path/to/boogie"
# tlc = "/path/to/tla2tools.jar"

[[models]]
file = "spec/counter.tla"
cfg = "spec/counter.cfg"
[models.exports]
step = "Counter.step/2"
```

Configuration paths are relative to the configuration file. Explicit command
line paths are relative to the current directory. A directory scan skips build,
dependency and artifact directories and does not follow nested symlinks.

For TLA+ source exports, arguments are mathematical integers and the return is a
kernel term record. A model can use `step(balance, delta).ival` after checking
`step_pre(balance, delta)`. Exports currently support nonrecursive pure functions,
bindings and `if`. [The counter model](demo/counter/counter.tla) imports the
generated `vex.tla` module. [The mailbox model](demo/mailbox/mailbox.tla) shows
explicit actors, mailboxes and interleavings. The author supplies runtime
abstractions, bounds and fairness assumptions. A TLC pass is a result for that
configured finite model, not an unbounded concurrency theorem.

The mailbox example checks eventual delivery under weak fairness of each
receiver. Removing fairness produces a temporal counterexample:

```sh
bin/vex model demo/mailbox/mailbox.tla --cfg demo/mailbox/unfair.cfg
```

## read the result

```sh
bin/vex check lib --json
bin/vex check demo/fail/post.ex       # exits 1, keeps an SMT counterexample
bin/vex model demo/fail/race.tla      # exits 1, keeps the lost-update trace
```

Every run gets a fresh artifact directory. `.vex/report.json` points to the last
result and contains source hashes, a verifier build fingerprint, tool versions,
coverage, contract dependencies, proof evidence and log paths. No proof-result
cache is enabled. A failed dependency makes its callers unproved. Missing tools,
timeouts, unknown results, empty selections and unsupported syntax cannot produce
a successful contract check. Exit codes: `0` successful selected checks, `1`
unmet verification obligations/models, `2` invalid input or setup.

## supported today

The `erlang.discrete.1` profile supports arbitrary integers, atoms, boolean atoms,
Elixir `nil`, proper/improper lists, tuples, exact equality, integer comparisons,
integer arithmetic with Erlang `div`/`rem`, type predicates, `hd`, `tl`, literal
`elem`, statement bindings, `if`, ordered `case` patterns, and contracted local/qualified
calls. Comparisons require integer operands; cross-type term ordering is outside
this profile. Runtime failures within supported operations are safety obligations.

Function heads currently use distinct named arguments and one clause. Put
patterns and dispatch in `case`. Ordinary macro expansion, aliases/imports,
compile hooks, floats, maps, binaries, exceptions, higher-order/dynamic calls,
NIFs, ETS, `spawn`/`send`/`receive`, and automatic OTP extraction are unsupported.
Pure transitions can be isolated and imported into explicit TLA+ models. The
[semantics](docs/semantics.md) and [trust](docs/trust.md) documents state the exact
boundaries and extension points.

## develop

```sh
bin/check.sh
```

The [design](docs/design.md) records the study of Verixir, Aristotle, formal-proofs,
Lynx and i5h, the reasons for each component, and the full extension plan. The
[protocol](docs/protocol.md) describes stage inputs and outputs. Paths, project
types and functions use lowercase names; mandatory ecosystem filenames such as
`Cargo.toml` keep their required spelling.

[Extension rules](docs/extend.md) explain how to add source constructs, Erlang
operations, backends and actor refinement without mixing the component boundaries.
The GitHub workflow runs the same setup and validation scripts on Linux.
See [validation](docs/check.md) for the executed checks and their scope.
