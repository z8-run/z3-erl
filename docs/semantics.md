# semantics

## discrete values

`erlang.discrete.2` uses `kernel::theory::datatypes` to declare `int`, `atom`,
`nil`, `cons`, and `tuple`, plus the finite `seq` sort with `empty` and `push`.
SMT-LIB, Boogie, Lean and TLA+ printers consume that description. A tuple wraps a
finite sequence of fields; tuple patterns check every field and its exact length. An
improper list uses an arbitrary tail. `[]` is list nil; Elixir `nil` is an atom.

Arithmetic uses unbounded integers. `div(a,b)` divides absolute magnitudes then
restores the sign, and `rem(a,b) = a - div(a,b)*b`. Noninteger operands and zero
divisors create safety obligations. Comparisons are the integer portion of
Erlang ordering; users must establish integer operands. Floats and cross-type
ordering have no encoding in this profile. Selectors have explicit defaults on
wrong constructors in every backend; safety obligations prevent relying on those
defaults as successful runtime operations.

Exact and nonexact equality coincide within the supported discrete profile.
`and`/`or` require a boolean left operand and return the selected operand.
`&&`/`||` and `if` use Elixir truthiness: only `false` and atom `nil` are falsey.
Short-circuited expressions impose no runtime safety requirement on the skipped
branch. Predicates themselves are total over the discrete term domain.

`is_list` returns true for both empty and nonempty cons cells, including improper
lists, matching OTP. `tuple_size` requires a tuple. `elem` accepts any integer
index with `0 <= i < tuple_size(t)`; indices are zero based and have no artificial
255 limit. Recursive sequence equations are shared by all proof targets. Size
positivity, sequence length and field extensionality lemmas are checked in
`kernel/lean/seq.lean` on the pinned toolchain.

## functions and scopes

Source parameters are universally quantified. Each requires expression must be
well-defined after preceding requires facts. The runtime guard further restricts the
domain. A guard operation error or a non-true result rejects that clause and
tries the next one. Guards are checked against the supported Elixir guard syntax.
Clause dispatch considers patterns and guards in source order, independently of
requirements. Calls must establish the requirements of the selected clause.

Assignments used as statements may rebind names. Bindings inside argument lists,
operators and conditions are rejected: Elixir's sibling-expression scope requires
a separate lowering rule. Move these assignments to preceding statements.
Pattern variables are fresh within a pattern and
repeated occurrences require equality. `case` clauses are considered in source
order, with prior matches excluded; uncovered paths create a coverage obligation.
Variables introduced in a case/if branch or ghost block do not escape that scope.
Nested match patterns and ordered function clauses use the same matcher. Pin
patterns remain unsupported. Head patterns can bind an alias, such as `[h | t] = xs`,
for referring to the complete original argument in contracts.

The contract's parameter names refer to original inputs, even if the body rebinds
them. A self call with those exact symbolic arguments in ensures denotes the
current return value. Other specification calls must name ghost functions whose
preconditions hold. Calls to executable functions use modular summaries after
checking their preconditions. Unknown callees and remote private calls fail.

Ghost functions and erased proof blocks can call verified total functions.
An erased computation cannot establish a fact using a partial runtime function's
postcondition. Recursive ghosts need a well-founded measure decreasing on every
call in their recursive component. Integer components are nonnegative; other
terms use structural size. Tuple decreases annotations denote lexicographic
components. `term_size(t)` explicitly measures a term, including integer leaves. Unfolding
instantiates a checked ghost body locally; the expanded recursive calls retain
their modular summaries. While verifying a definition, unfolding its own
recursive component is rejected: otherwise a future instance could justify its
own unproved contract. Decreasing ghost calls supply the induction hypothesis.
Missing termination or any failed dependency invalidates
the owner's final success. Executable recursion can instead be explicitly partial.

## proof statements

Assertions check definedness and truth before contributing a fact; an optional
literal message is retained in the plan and failure diagnostic. `forall` and
`exists` bind profile terms lexically. Their predicates must be boolean and
well-defined for every bound value, including for existential specifications.
Quantified expressions can also compute values in ghost functions and ghost
blocks. Their definedness is checked before use; executable bodies reject them.
Calls in such values must be total and outside the enclosing recursive component;
recursive quantified definitions currently lack a checked decrease rule and are rejected.
Logical substitution renames binders when needed to avoid variable capture.

`havoc` introduces fresh ghost values. `block` verifies a local proof and restores
both its environment and facts. `assume` checks definedness and boolean type,
records an admission, and adds its formula. Admissions make the containing
contract and all dependent contracts conditional, even when all solver formulas
pass. They cannot make `check` succeed. Ordinary branch and contract assumptions
are generated by the verification rules and are not admissions.

Ghost statements are removed from sequences in both the verifier and runtime
macros. A trailing ghost leaves the preceding runtime result intact; a body
containing only proof statements returns `nil`. Use `defv`/`defvp` for proof
statements; ordinary `def`/`defp` contracts are accepted only without such code.

## effects and concurrency

The source profile is pure. There is no hidden approximation of a BEAM mailbox,
process dictionary, exception, ETS table, scheduler or native function. Unsupported
effects fail at the frontend/graph boundary.

Concurrency is explicit in TLA+. The included examples distinguish an atomic
transition model, a bounded mailbox model, and a lost-update race. None is an
extracted OTP runtime. `kernel/lean/actor.lean` gives a generic inductive invariant
theorem and a simulation theorem whose premise must connect actual concrete
steps to abstract steps. It makes no fairness or liveness claim.
