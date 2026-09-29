# extend

Keep language meaning in `kernel`, source transformations in `flow`, target
syntax and process execution in `back`, and configuration in `cli`. Backend
printers must not learn Elixir AST details. New adapters consume the public
types in `kernel`; `flow` and `back` do not depend on each other.

## add an operation

1. Specify its domain, result and failure behavior in `kernel/src/bif.rs` using
   the shared algebra. A runtime error must produce a safety condition.
2. Add syntax recognition in `front/lib/read.ex`. Fully qualified Erlang calls
   need their own checked name/arity mapping; Elixir operators are not all BIFs.
3. Compare results with actual OTP and run the same obligations through Z3,
   Boogie and Lean. Add false contracts that fail as well as true ones that pass.
4. Update `docs/semantics.md` and the profile version when meaning changes.

If the existing algebra suffices, no backend-specific operation is needed. A new
primitive instead needs a sort, semantics in every target, conformance checks,
and a documented trust argument. A new value constructor starts in
`kernel/src/theory.rs`; `bin/vex kernel` regenerates the Lean datatype.

## add a source construct

Define its JSON shape in `kernel/src/ir.rs`, parse it without evaluating user
code, validate its binding scope, and implement its symbolic execution rule.
Decide explicitly how it affects safety, termination, dependencies and source
spans. Unknown behavior must fail at this boundary. Add runtime comparison and
negative integration tests before advertising support. Source-to-TLA exports
may reject a construct supported by contract checking; the two capabilities are
separate and must be documented.

## add a backend

Consume `kernel::vc::vc` or `kernel::model::export`; do not duplicate the BIF
table. Emit inspectable artifacts, execute through `back::run`, preserve logs,
and classify absence, malformed output, timeout and unresolved proof separately.
Success must include positive evidence that the intended obligation was checked.
Add it to explicit engine selection before deciding whether automatic fallback
should use it. A solver cannot widen the source semantic profile.

## grow concurrency support

Start with explicit TLA+ state, atomic actions, message order, failures, bounds
and fairness. Import pure transition operators where supported. Next define an
operational actor profile and a concrete-to-abstract state relation. Use the
simulation and invariant rules in `kernel/lean/actor.lean` to discharge that
relation's obligations. Only after this connection exists should source actors
receive a refinement claim. A successful finite TLC exploration by itself never
establishes the connection.

## required validation

`bin/check.sh` runs formatting, Rust lint/build/tests, Elixir compilation and
DSL tests, the pinned Lean library, real-tool integration and OTP conformance,
then the configured example project. Its false-postcondition, failed-dependency,
ghost, missing-tool, unsupported-syntax and lost-update cases must remain failing
verification examples while passing the test suite.
