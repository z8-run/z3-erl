# trust

## what a successful check means

For each selected function, the generated obligations establish its stated
return contract and supported operation safety under `erlang.discrete.2` and the
recorded precondition/guard. Dependencies must also pass. A checked measure gives
termination for supported recursive calls; otherwise recursive executable code
is explicitly partial. An inconsistent precondition can make a contract vacuous;
vex does not currently prove preconditions inhabited.

The tool does not claim that unannotated functions, initialization code or
arbitrary actors have been proved. `--strict` applies to the discovered inventory,
not to external dependencies or a running deployment. `model` checks temporal
models without checking source contracts. `check` checks both contracts and the
models in the configuration.

## trusted components

| component | assumption |
| --- | --- |
| Elixir quoted-AST parser and frontend | selected source constructs lower faithfully; source is not executed |
| Rust graph, symbolic execution, simplification and VC generation | obligations correctly express source behavior and modular proof rules |
| common kernel and printers | Erlang's documented discrete semantics are faithfully represented in each target |
| Z3 | reported SMT results are correct; no SMT proof certificate is replayed in Lean |
| Boogie | generated programs and its VC pipeline are correct; its default solver here is Z3 |
| Lean | kernel checking is correct; only `propext`, `Classical.choice`, `Quot.sound` are allowed axioms |
| Elixir compiler and BEAM | compiled code follows the selected source semantics; vex does not prove either compiler |
| TLC | exploration, hashing and configured temporal checks are implemented correctly |
| model abstraction | TLA+ actions, atomicity, mailboxes, failures, bounds and fairness accurately represent the intended shell |

Kernel proofs cover the propositions actually written in `kernel/lean`, including
Hoare composition and reachability invariant preservation. They are not a theorem
that the Rust translator or complete Erlang semantics is correct. A generated
Lean obligation is stronger evidence about the formula than an SMT answer, but
still relies on faithful source translation.

## representation details

The datatype contains integers, atoms, list nil, cons and tuples. Atom identities
are interned injectively in the plan; `false`, `true` and `nil` have fixed identities
0, 1 and 2. This representation does not supply lexicographic atom ordering.

SMT and Lean use mutually inductive terms and finite tuple-field sequences.
Boogie uses constructor tags, projections, reconstruction and disjointness axioms
from the same description. The recursive size and indexing equations come from
the shared kernel. Additional induction lemmas for length, positive size and
sequence extensionality have audited Lean proofs. The first-order Boogie encoding
can still have nonstandard models; a counterexample need not describe an actual
finite Erlang value. Tuples no longer admit improper field spines.

All verification statements concern values in this profile. Unmodeled runtime
values are not encoded as arbitrary integers. Imported TLA+ source functions have
integer arguments and explicit domain predicates. TLC may only explore a finite
instance, with fingerprint collision risk inherent to the checker. Source exports
do not prove an actor shell refines its TLA+ specification.

## evidence and failures

A condition's success is separate from its owner's success. Proof assertions and
callee summaries may be used while checking later obligations, but every earlier
obligation and every transitive dependency must pass before the contract is
reported proved. Recursive ghost definitions require a well-founded measure;
their equations are unfolded locally only. Source `assume` directives are explicitly recorded admissions. The owner and all
transitive callers are reported `conditional`, never `proved`, and `check` fails.
A passing per-condition Lean theorem can still have an admitted source formula
as a hypothesis; the contract report must therefore be consulted.

Solver output is classified using exit status and complete success markers.
Timeouts kill the child process group. Logs and artifacts are preserved. Results
are never loaded from an old artifact directory. The verifier build fingerprint,
input hashes, atom table and tool versions permit inspection of exactly what was
checked. Files changed during a source check invalidate that check.

Lean proof files are trusted local developer input, like other Lean source: the
Lean elaborator can execute metaprograms. They are not a sandbox for hostile code.
The audit prevents mathematical shortcuts through `sorryAx` or extra axioms.

## roadmap to a smaller trusted base

1. Validate each lowering pass against a small operational semantics.
2. Prove a VC soundness theorem and connect its actual Rust output to the theorem.
3. Add proof-producing Core Erlang import with source/BEAM provenance.
4. Add runtime profiles with differential tests and preservation lemmas.
5. Generate and discharge shell-to-model refinement obligations.

Conformance and regression tests provide engineering evidence; they are not
substitutes for these missing metatheorems.
