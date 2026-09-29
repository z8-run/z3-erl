# design

## purpose

`vex` checks explicit contracts in Elixir projects and temporal properties in
TLA+ models. It combines automation with proof engineering rather than making
every program execute inside Lean. A project may contain millions of lines;
verification is decomposed into small contracts and model boundaries. No result
means that arbitrary, unselected Elixir or the entire BEAM runtime was proved.

This design was settled before implementation. Its first implementation must
run every stage on real tools, include failing examples, and state unsupported
features. A backend that is absent, times out, or cannot interpret its input
cannot produce a successful verification result.

## lessons from the reference projects

The following local projects were read as design references, not dependencies:

| project | examined | lesson used |
| --- | --- | --- |
| `../verixir-project` | `verixir.ex`, `l2_code.ex`, built-in specs, Fibonacci example | keep `@verifier`, `defv`, `defvp`, `defvg`, `ghost`, `assert`, and `unfold`; separate checking from compilation |
| `../aristotle` | erlean design, core syntax, contracts, actor invariants | distinguish values, exceptions, unsupported behavior, partial correctness, termination, and scheduling assumptions |
| `../formal-proofs` | CPU semantics and Hoare rules | state the proposition before automating it; keep semantics separate from code-specific proofs |
| `../lynx` | term API, translator, translation regressions | preserve provenance, track callees, reject missing semantics, and audit Lean axioms |
| `../i5h` | kernel trait, design, trust document | put pure decisions behind a small boundary; prove transitions and state the shell's obligations separately |

Verixir's Boogiex is inspired by Boogie; it is not the Boogie executable. This
project supports actual Boogie programs and the Boogie verifier.

## bounded contexts and directory tree

```text
bin/                 entry point, setup and verification scripts
docs/                design, protocol, semantics, trust, extending
front/               Elixir package: contract syntax and quoted-AST reader
kernel/              shared Rust library: source IR, logic, Erlang operations
  lean/              generated term theory and hand-written semantic lemmas
flow/                validation, symbolic execution, contract obligations
back/                SMT-LIB, Boogie, Lean, TLA+ printers and tool execution
cli/                 paths, configuration, scheduling, reports, commands
demo/                passing and failing source/spec/proof projects
tests/               cross-component and real-backend regression checks
```

Dependencies point downward: `cli -> back + flow -> kernel`. `front` produces a
versioned JSON document consumed by `kernel`; it does not import Rust internals.
`kernel` has no process launching, file discovery, solver dependency, or CLI.
`flow` has no knowledge of solver syntax. `back` never translates Elixir.
All project-controlled paths and names are short and lowercase. Language-required
external names (`Int`, `Mix.Project`, TLA+ standard modules) retain their spelling.

Rust owns data validation, graphs, artifact management and bounded parallel jobs.
Elixir owns parsing its own syntax and the runtime contract macros. Lean owns
proof terms and semantic lemmas. Existing Z3, Boogie and TLC own solver search.
There is no handwritten Elixir parser and no custom SMT or temporal solver.

## pipeline interfaces

| stage | input | output | failure |
| --- | --- | --- | --- |
| discover | paths or `vex.toml` | sorted source inventory | missing paths, empty selection |
| read | source files | versioned modules, functions, specs, spans, coverage | syntax or unsupported source construct |
| validate | source IR | resolved function graph | ambiguity, missing callee, ghost escape, malformed IR |
| lower | graph plus `kernel` operations | independently named verification conditions | unsupported operation, resource limit |
| emit | condition | `.smt2`, `.bpl`, `.lean` | unsupported target capability |
| check | artifact plus tool settings | evidence with status and diagnostic | missing tool, timeout, malformed output |
| model | `.tla` plus `.cfg` and optional source exports | TLC run and trace | state-space or specification failure |
| report | evidence and source inventory | text/JSON report and artifact manifest | never hide incomplete obligations |

Each condition contains its owner, source span, kind, universal variables,
hypotheses, conclusion, function dependencies and stable content identity.
Reports retain inputs, hashes, tool versions, generated artifacts and raw output.
No result is cached merely by filename, and emitting a file is not verification.

## one kernel

The initial semantic profile is discrete Erlang values: arbitrary integers,
atoms (including boolean atoms and Elixir `nil`), proper/improper lists, and
tuples. `[]` and `nil` are distinct. Exact equality is constructor equality.
Arithmetic is unbounded integer arithmetic. `div` truncates toward zero and
`rem` has the dividend's sign. Division by zero and type errors become safety
obligations. Guards restrict the function domain. Boolean short-circuiting and
Elixir truthiness are different operations.

The kernel defines a small sorted logical algebra and a datatype description.
Operations are built from that algebra once. All printers consume it; a backend
does not maintain a separate table of Erlang arithmetic semantics. Datatype
declarations for SMT, Boogie, and Lean are derived from the same description.
The Lean term library proves selected properties of that representation.
Conformance tests compare supported operations with the actual Erlang runtime.

Floats, maps, binaries, arbitrary macros, NIFs, dynamic dispatch, ETS and actor
runtime effects require additional profiles. They are diagnosed, never silently
replaced by unconstrained values. New profiles extend the same boundaries.

## contracts and modular reasoning

The public Elixir API is `use :vex` with Verixir-style annotations. Parsing is
read-only: the checker does not compile or execute project code. Runtime macros
erase proof statements and ghost functions. Verification and application startup
are independent commands.

Every call checks the callee's precondition before using its postcondition.
Results depend on successful verification of their callees. Recursive executable
contracts express partial correctness unless a decreasing integer measure is
checked. Ghost functions used as mathematical functions must terminate: recursive
ghost calls require a nonnegative, strictly decreasing measure. A single integer
argument can supply the default measure; other cases use `@verifier decreases`.
`unfold` adds a local, guarded instance of a ghost definition. It cannot introduce
an unchecked global recursive axiom. Assertions generate obligations before their
facts are available to following statements. Arbitrary `assume` is not a proof API.

The return expression `f(original_args)` in `ensures` denotes the current result.
Other mathematical calls name checked ghost functions. Binding and branch scope
follow Elixir; erased ghost assignments must never affect executable values.

## engine selection and evidence

Direct Z3 checks the negation of each implication. `unsat` means discharged under
the recorded encoding; `sat` records a counterexample to that condition; `unknown`
remains unresolved. Boogie receives equivalent assumptions and assertions in a
real procedure and uses its own VC machinery. It is an alternative route, not an
independent oracle when it also invokes Z3.

Lean receives the same universally quantified proposition, never an axiom saying
that a solver succeeded. Automatic tactics or a user proof term must inhabit that
proposition. The checker audits the resulting theorem's axioms. Only
`propext`, `Classical.choice`, and `Quot.sound` are permitted; `sorryAx`, custom
axioms and native-decision trust extensions are rejected. Every Lean invocation
explicitly selects `leanprover/lean4:v4.28.0`. No Mathlib dependency is required.

`auto` tries Z3 first and escalates unresolved obligations to Boogie and Lean.
Counterexamples are never hidden by fallback. Explicit engine selection is
available for reproducibility. The report distinguishes SMT evidence from Lean
kernel-checked evidence and records partial versus total contracts.

## concurrency and TLA+

TLA+ is the system specification boundary: process state, mailboxes, atomic
actions, failures, fairness and bounds are explicit in the model. TLC checks the
specified finite instance and retains counterexample traces. A passing TLC run
is a model result, not an unbounded proof of Elixir concurrency.

Source exports can generate TLA+ operators for supported nonrecursive pure
transitions. Hand-written TLA+ models import those operators and their domain
predicates. This links the decision logic to source while the model author owns
the abstraction of runtime effects. Generic Lean reachability/invariant rules
provide the separate path to unbounded invariant proofs. Merely listing a source
file beside a model is not refinement evidence.

## validation gates

1. Parse and compile the DSL; compare runtime behavior with ghost erasure.
2. Check arithmetic, matching, call preconditions, recursion, and scope using both
   passing and deliberately false contracts.
3. Run actual Z3, Boogie and Lean against generated obligations.
4. Audit Lean proof dependencies and reject `sorry`/extra axioms.
5. Run passing and failing TLC models, including one using a source export.
6. Compare discrete arithmetic and container operations with Erlang.
7. Check timeouts, missing tools, malformed output, empty projects and deterministic
   artifacts. Run Rust formatting, linting and tests plus Elixir and Lean tests.

## extension order

Stabilize source/logic/report protocols first. Add Core Erlang as another front
end without replacing the kernel or backends. Extend verified runtime profiles
(maps, binaries, exceptions) with conformance cases. Add OTP actor abstraction and
refinement obligations before claiming automatic verification of `GenServer`,
selective receive or distributed supervision. Add proof-producing translation
passes to reduce the trusted base. Incremental proof reuse requires transitive
dependency hashes and tool/profile identities, not just timestamps.

## primary references

- [Boogie](https://github.com/boogie-org/boogie): intermediate verification language,
  VC generation, and Z3 backend.
- [Z3 arithmetic](https://microsoft.github.io/z3guide/docs/theories/Arithmetic/):
  arithmetic fragments and possible unknown results.
- [Erlang expressions](https://www.erlang.org/doc/system/expressions.html): runtime
  operations, matching, guards and arithmetic.
- [TLC](https://docs.tlapl.us/using:tlc:start): explicit-state model checking.
