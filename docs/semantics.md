# semantics

## discrete values

`kernel::theory::terms` declares `int`, `atom`, `nil`, `cons`, and `tuple` once.
SMT-LIB, Boogie, Lean and TLA+ printers consume that description. A tuple wraps a
list of fields; tuple patterns check every field and the terminating nil. An
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

## functions and scopes

Source parameters are universally quantified. Each requires expression must be
well-defined after preceding requires facts. The guard further restricts the
domain. This profile conservatively requires guard operations to be well-defined;
it does not yet model guard exceptions falling through to another function clause.

Assignments used as statements may rebind names. Bindings inside argument lists,
operators and conditions are rejected: Elixir's sibling-expression scope requires
a separate lowering rule. Move these assignments to preceding statements.
Pattern variables are fresh within a pattern and
repeated occurrences require equality. `case` clauses are considered in source
order, with prior matches excluded; uncovered paths create a coverage obligation.
Variables introduced in a case/if branch or ghost block do not escape that scope.
Pin patterns, multiple head clauses and destructuring arguments remain unsupported.

The contract's parameter names refer to original inputs, even if the body rebinds
them. A self call with those exact symbolic arguments in ensures denotes the
current return value. Other specification calls must name ghost functions whose
preconditions hold. Calls to executable functions use modular summaries after
checking their preconditions. Unknown callees and remote private calls fail.

Ghost functions and erased proof blocks only call other ghosts. In particular,
an erased computation cannot establish a fact using a partial runtime function's
postcondition. Recursive ghosts need a nonnegative
integer measure decreasing on each call in their recursive component. Unfolding
instantiates a checked ghost body locally; the expanded recursive calls retain
their modular summaries. While verifying a ghost definition, unfolding its own
recursive component is rejected: otherwise a future instance could justify its
own unproved contract. Decreasing ghost calls supply the induction hypothesis.
Missing termination or any failed dependency invalidates
the owner's final success. Executable recursion can instead be explicitly partial.

## effects and concurrency

The source profile is pure. There is no hidden approximation of a BEAM mailbox,
process dictionary, exception, ETS table, scheduler or native function. Unsupported
effects fail at the frontend/graph boundary.

Concurrency is explicit in TLA+. The included examples distinguish an atomic
transition model, a bounded mailbox model, and a lost-update race. None is an
extracted OTP runtime. `kernel/lean/actor.lean` gives a generic inductive invariant
theorem and a simulation theorem whose premise must connect actual concrete
steps to abstract steps. It makes no fairness or liveness claim.
