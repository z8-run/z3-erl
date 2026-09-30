# coverage

This document lists the source and proof features that vex supports, where each
one is implemented, and the semantic rules behind them. [semantics](semantics.md)
states the exact meaning; [trust](trust.md) states what a result claims.

## features

| area | supported | implementation |
| --- | --- | --- |
| solver interface | typed formulas and declarations, one isolated solver state per obligation, unsat-of-negation checking, strict classification of failure and unknown, quantified formulas | `kernel::logic`, `back::print`, `back::run`, `back::solver` |
| values | integers, atoms (including booleans and `nil`), proper and improper lists, tuples; strict equality; integer arithmetic with Erlang `div`/`rem`; integer comparisons | `kernel::theory`, `kernel::bif` |
| predicates and access | `is_integer`, `is_atom`, `is_tuple`, `is_list`, `is_boolean`, `is_nil`, `tuple_size`, `hd`, `tl`, and `elem` with a symbolic, bounds-checked index | `kernel::bif` |
| expressions | compound constructors, modular calls, short-circuit `and`/`or`/`&&`/`||` | `flow::exec`, `flow::spec` |
| proof statements | `ghost`, `assert`, `unfold`, `assume` (ghost only, reported), `havoc` (ghost only), `block` (local proof scope) | `flow::proof`, `front/lib/vex.ex` |
| termination | nonnegative integer measures, structural `term_size`, lexicographic tuples, one measure per recursive clause, decrease across the whole recursive component | `flow::rank`, `flow::exec` |
| matching | variables, repeated variables, literals, tuples of exact length, lists with tails, nested `=` patterns, rebinding, ordered `case` with coverage, isolated branch scope | `flow::pattern`, `flow::exec` |
| function clauses | ordered clauses, argument patterns, one guard per clause, per-clause contracts | `front/lib/read.ex`, `flow::clause` |
| quantifiers | `forall` and `exists` over profile values in contracts and assertions | `flow::spec` |
| runtime erasure | proof statements and ghost functions do not exist in compiled code | `front/lib/vex.ex` |

Outside the profile: floats, maps, binaries, exceptions, higher-order and dynamic
calls, pin patterns, cross-type term ordering, and concurrency in source code.
Concurrency is modeled explicitly in TLA+.

## rules by component

* `kernel`: a genuine finite sequence sort for tuple fields, alongside Erlang
  terms; typed quantifiers; structural size, sequence length and lookup. Shared
  recursive equations feed SMT and Boogie and definitions feed Lean. Tuples must
  not admit improper field lists. Semantic changes increment the profile.
* `front`: collect ordered clauses and their annotations without executing user
  code. Keep parsing and runtime proof erasure in agreement. Increment the source
  protocol for the clause representation.
* `flow`: compute runtime selection from argument patterns and guards, excluding
  earlier matches. Requirements constrain the selected clause; they never select
  a later clause. Verify each reachable body and use conditional postconditions
  at calls. Guard failure falls through. Measures belong to clauses and must
  decrease across the whole recursive component.
* `back`: print identical propositions in all three targets. Quantified variable
  scopes must not become free variables. Recursive theory lemmas used by solvers
  must also have Lean proofs on `leanprover/lean4:v4.28.0`.
* evidence: explicit source `assume` is admitted information. Record it, propagate
  its effect to callers, and do not report an unconditional proof or successful
  `check` for a result depending on it. Generated branch/precondition assumptions
  remain justified by the verification rules.

## semantic decisions

- An obligation is discharged when the negation of its implication is
  unsatisfiable.
- An empty block evaluates to the atom `nil`, which is distinct from the empty
  list `[]`.
- `elem/2` indices are zero-based: a tuple of size n accepts 0 through n−1.
- Requirements never influence clause selection. A failed contract must not let
  a runtime clause be skipped.
- `assume`, including `assume false`, makes a result conditional. It can never
  produce an unconditional proof.
- These rules do not prove the translation itself correct; see [trust](trust.md).

## validation

`demo/paper.ex` and `tests/test_papers.py` run on real Z3, Boogie and Lean. They
cover clause dispatch and requirements that cannot skip a runtime clause,
rejection of omitted clauses and unerased proofs, reported and propagated ghost
assumptions, local proof facts and `havoc` scope, quantifier scope and
definedness, tuple bounds with large indices, nested matches with repeated
variables, lexicographic measures, quantified tuple extensionality, and runtime
behavior after ghost erasure. Variables named `ghost`, `assert`, or other proof
keywords remain ordinary variables in both verified and compiled code.

## source review

The implementation was checked against the complete local copies of
[Program Verification in Elixir](adrian-enriquez.pdf), Adrián Enríquez Ballester,
2022, and [The Dafny and Boogie Verification Tools](dafny-boogie.pdf), Philipp
Schepper, 2018.
Page numbers below refer to printed pages; the Elixir thesis has twelve pages
of front matter before page 1.

| source | requirement | realization |
| --- | --- | --- |
| Elixir, chapter 4, pp. 19–25 | solver interaction, declarations, scoped assumptions, validity and failure | typed kernel formulas, isolated per-obligation solver processes, retained artifacts and strict result classification |
| Elixir, §5.2.1–5.2.2, pp. 28–31 | discrete terms, integer operators, short-circuit booleans, lists, tuples, predicates, equality and selectors | common datatype and BIF semantics; symbolic tuple indices; quantified field extensionality |
| Elixir, §5.2.3, pp. 32–37 | modular calls, assertions, assumptions, fresh variables, local blocks and unfolding | symbolic paths and `flow::proof`; explicit admissions make results conditional |
| Elixir, §5.2.4, p. 36 | structural term size | shared recursive equations and audited Lean positivity lemmas |
| Elixir, §6.1–6.2, pp. 41–50 | nested patterns, rebinding, ordered cases and function clauses, contracts, ghost erasure | quoted-source frontend, shared matcher, clause dispatch and runtime macros |
| Dafny, §§3–4 | contracts, ghost state, mathematical predicates, termination measures, intermediate verification and diagnostics | ghost functions and quantifiers, structural and lexicographic decreases, emitted Boogie programs, source-located obligations and assertion messages |

Several distinctions matter when applying the papers. Runtime dispatch uses
patterns and guards; preconditions never cause a matching clause to be skipped.
Tuple indices are zero based. An empty Elixir block returns atom `nil`, distinct
from `[]`. Validity requires the negation to be **unsatisfiable**. Explicit
`assume` is admitted information, even when its formula is `false`.

The thesis limits its verified source fragment to sequential code and excludes
exceptions, higher-order functions and pin patterns (§1.2.2, p. 3). Its integer
comparison specification does not implement general Erlang term ordering
(§5.2.2, p. 30). Appendix A's comprehensions, maps and pins construct an SMT
problem; they are not additional verified L2 source constructs. The internal
solver interface serves the same verification role as L0; vex does not reproduce
the thesis's separate interactive Elixir SMT-LIB binding API. Dafny's mutable
heap, arrays and imperative loops are not Elixir source features implemented
here. These boundaries prevent a feature comparison from implying support for
arbitrary Elixir or every Dafny program.

Supported syntax does not guarantee automatic proof discovery. Unknown results
and timeouts remain unproved; difficult obligations can use checked Lean proofs.
