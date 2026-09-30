# protocol

## components

| component | public input | public output |
| --- | --- | --- |
| `front/read.exs` | source paths, or `--manifest paths.json --out source.json` | `vex.ir.2` |
| `kernel::ir` | JSON source document | validated Rust data shapes |
| `flow::lower` | `&kernel::ir::source` | `flow::plan` |
| `back::print` | `&kernel::vc::vc` | SMT-LIB, Boogie or Lean text |
| `back::solver::check` | condition, engine, settings | evidence |
| `flow::model::export` | source world and function id | pure term expression plus domain predicate |
| `back::tla::check` | TLA+, configuration, source exports | temporal evidence and raw trace |
| `cli` | paths/configuration/options | `vex.report.1`, artifacts, exit code |

## source

`version` is exactly `vex.ir.2`. `files` contains absolute paths and SHA-256
digests. `functions` contains module/name, canonical argument names, visibility,
source span and ordered `clauses`. Each clause carries argument patterns, requires,
ensures, guard, optional decreases, body and its own source span. `skipped` inventories unannotated
definitions. Integers are decimal strings, never JSON machine numbers. Atoms are
UTF-8 names. `node` is a tagged object (`kind`) with a source `line`.

The authoritative schema is `kernel/src/ir.rs`; serde rejects unknown fields for
data records. The frontend recognizes source syntax without expanding arbitrary
macros. All clauses of a selected function must be included; dispatch preserves their
source order and uses the same pattern rules as case expressions.
Function ids are `Module.name/arity`. Elixir aliases, including an explicit
`Elixir.` prefix, resolve to the same module identity. Non-Elixir atom modules
retain a leading colon (`:counter.step/1`) so they cannot alias Elixir modules.

## obligations

The logical algebra has `term`, `seq`, `int`, and `bool` sorts. Primitive constructors,
selectors and predicates come from `kernel::theory`. `kernel::bif` expresses every
supported runtime operation using this algebra and an explicit safety predicate.

A condition includes `id`, `owner`, `kind`, `span`, optional assertion `message`, `hypotheses`, `goal` and `deps`.
Its meaning is universal closure of `hypotheses => goal`. Condition kinds identify
domain well-formedness, operation safety, assertions, call preconditions, case
coverage, measures, decreases, matching and postconditions. Stable ids hash the
condition, semantic profile and dependency identities. There is no persisted proof
cache; dependencies are always rechecked for a run.

Source spans are provenance metadata, excluded from condition identities so that
proof files work across checkout paths and unrelated source-line changes.

All target-language names are injectively encoded from UTF-8 bytes. User module,
function and variable names are never inserted as executable solver syntax.

## evidence

Evidence carries a condition id, engine, status, artifact path, log path, detail
and elapsed time. Status is one of `proved`, `counterexample`, `unknown`, `timeout`,
`unavailable`, `error`, or `model_checked`. Only TLC uses `model_checked`; it
denotes successful finite model checking and is distinct from a discharged
contract. The report also preserves the model scope and configuration.

The CLI propagates unsuccessful contracts through dependency closure, including
recursive components. An emitted file, a zero-error run that verifies no Boogie
procedures, an empty source selection, or a Lean file without the audit marker
does not count as a proof. `all` requires all requested routes to pass.

Contracts also include explicit source admissions with their spans and formulas.
A contract status of `conditional` means its VCs passed but it or a dependency
uses `assume`. Conditional results exit unsuccessfully. Quantifier binders belong
to their expression scopes and do not appear among a VC's free input parameters.
