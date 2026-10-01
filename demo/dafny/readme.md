# dafny translations

Executable Elixir adaptations of the Dafny integration examples at revision
`5f717bf447b19d38cad1b69b1bf9a9f102feccfb`. Each example is added only after its
contracts pass real solvers. Upstream code is MIT licensed; see [license](license).

The progression is maximum, list and environment operations, expression
interpretation, rewriting, stack execution and compilation, then parsing.
Proofs belong to the sample modules; language fixes belong to `front`, `flow`
or the shared `kernel`. No sample introduces an assumed theorem or disables a
failed obligation.

```sh
make check-dafny sample=max
make check-dafny sample=compile engine=boogie
make check-dafny sample=parse
make check-dafny sample=all
make test-dafny
make test-dafny case=test_dafny.dafny.test_compiler
bin/vex check --config demo/dafny/vex.toml
```

The Makefile selects dependencies for each sample. `engine`, `jobs`, `timeout`
(milliseconds per obligation), and `out` can be overridden. `make report`
summarizes the last run; use the same `out` override to inspect a separate run.
Make defaults to 10 seconds for SMT and 30 seconds when Lean is selected, allowing
for cold startup. An explicit `timeout` always takes precedence.
The regression suite uses both direct Z3 and Boogie, checks real Elixir behavior,
and requires incorrect mutations to remain unproved. These complex examples do
not claim to pass the default Lean automation; the verifier still supports
supplying audited Lean proofs on `leanprover/lean4:v4.28.0`.

Dafny immutable sequences become Elixir tuples when indexing matters and lists
when recursive construction matters. Loops become recursive helpers whose
preconditions state the loop invariants and whose decreases annotations state
the remaining work. Algebraic datatypes become tagged tuples. Compiler variable
names use integer ids and contexts use tuples of integer register values;
out-of-range ids read as zero, matching the original unbound-variable behavior.
The statement compiler (assignment/output) and C#/ANTLR interoperability are
outside this translation. Likewise, this directory does not port the separate
`induction-principle-code` framework or the general higher-order parser API.

## maximum

[Source](https://github.com/dafny-lang/dafny/blob/5f717bf447b19d38cad1b69b1bf9a9f102feccfb/Source/IntegrationTests/TestFiles/LitTests/LitTest/examples/maximum.dfy).
`max.ex` preserves membership, the universally quantified upper bound and the
uniqueness lemma, for arbitrary nonempty tuples of integers. `scan/3` carries
the original loop invariants and decreases the unvisited length.

## lists

[Source](https://github.com/dafny-lang/dafny/blob/5f717bf447b19d38cad1b69b1bf9a9f102feccfb/Source/IntegrationTests/TestFiles/LitTests/LitTest/examples/Simple_compiler/Compiler.dfy).
`list.ex` translates `LinkedList.Concat` to `:seq.append/2`, specified against
the ghost `cat/2`, and proves associativity. Elements may be arbitrary profile
values. The recursive `proper/1` predicate excludes improper tails; `is_list/1`
alone does not establish that property in Erlang.

## environments

`env.ex` adapts the compiler's context lookup. Its ghost `valid/1` computes a
quantified predicate; `get/2` guarantees the selected register value or zero
for an unbound id, with explicit bounds and integer-result guarantees.

## expressions

`expr.ex` adapts `DafnyAST.interpExpr`. Constants, register reads, addition and
subtraction use tagged tuples. A recursive ghost predicate checks the entire
tree. `eval/2` proves equality with the mathematical `value/2` for every valid
tree and context, with structural termination. Runtime tests also compare 212
tree/context pairs with an independent evaluator, including a depth-40 tree.

## rewriting

`rewrite.ex` adapts `Rewriter.simplifyExpr`: `0 + x`, `x + 0`, and `x - 0`
collapse after recursively simplifying their operands. `correct/2` proves
semantic preservation for an arbitrary environment by structural induction;
the executable `run/1` equals the verified ghost normalizer. The context is a
parameter of the erased theorem, so the runtime rewriter needs only the tree.
The mutation regression deliberately introduces the false rule `0 - x = x`.

## stack machine

`stack.ex` covers constant/register pushes and addition/subtraction, including
the upstream no-op behavior on stack underflow. Programs execute tail first,
as in `interpProg'`; this order is essential for noncommutative operations.
The recursive predicates establish proper instruction lists and integer stacks.
`join/2` proves concatenation preserves program validity, and `concat/4` proves
the execution composition law used by the compiler proof.

## compiler

`compile.ex` adapts `compileExpr` and `compileExprCorrect'`. It proves that
executing compiled code pushes the interpreted value onto any valid initial
stack and leaves the remainder of that stack intact, for any valid context.
`:compiler.run/1` compiles to the ghost `code/1`; `correct/3` supplies the structural
induction and uses the stack composition lemma. Arithmetic proof helpers keep
recursive obligations small. Runtime checks also exercise the complete
rewrite → compile → execute pipeline. The negative test reverses instruction
order in both the model and executable compiler: their agreement survives,
but the semantic correctness theorem must fail.

## parser

[Source](https://github.com/dafny-lang/dafny/blob/5f717bf447b19d38cad1b69b1bf9a9f102feccfb/Source/IntegrationTests/TestFiles/LitTests/LitTest/examples/parser_combinators.dfy).
`parse.ex` specializes `Char`, `Either`, `Concat`, `Epsilon`, and `EOS` for the
fixed `Parentheses` grammar, replacing closures with direct calls. Input is a
tuple of character codes; `run/1` returns a nesting count or `:error`. The
proof establishes termination, cursor bounds, exact consumed length and that
every accepted input consists of `n` opening followed by `n` closing parentheses.
The quantified `good/3` predicate specifies the consumed slice; `wrap/3` proves
the recursive step. Completeness (accepting every valid input) is covered by
runtime tests, not a universal theorem here. Tests enumerate all 3,280 strings
of length 0–7 over `(`, `)`, and `x`, plus deeper and malformed cases. Mutations
remove recursive progress or corrupt the returned count.

## verifier change

The environment and grammar predicates exposed a missing case: quantified
expressions could appear in specifications but could not return values from
ghost code. `flow::proof::quantified` now handles these values using the shared
specification semantics, including boolean-domain checks and dependency tracking.
Runtime quantifiers remain rejected. Calls into the same recursive component are
also rejected because the specification evaluator does not check their decreases.
Regressions cover definedness, non-boolean predicates, erasure, and both direct
and mutual recursive quantified definitions. No solver axiom was added.
