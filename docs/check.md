# check

Validated on macOS on 2026-09-30. `bin/setup.sh` and `bin/check.sh`
completed successfully using real installed backends.

| check | result |
| --- | --- |
| Rust formatting and Clippy with warnings denied | passed |
| Rust tests | 5 passed |
| Elixir formatting, compilation and DSL tests | 2 tests passed |
| Lean library, including axiom audits | passed on `leanprover/lean4:v4.28.0` |
| CLI integration and runtime conformance | 38 tests passed |
| clause and proof demo | 14 contracts passed in Z3, Boogie and Lean |
| Fibonacci ghost, accumulator and public function | 51 obligations passed in each of Z3, Boogie and Lean; 153 results |
| OTP integer conformance | 45 signed, zero and large-integer cases checked through all three backends |
| OTP container conformance | 19 list, tuple, predicate, size, selector and equality cases checked through all three backends |
| configured demo project | 9 contracts, 69 Z3 obligations, 2 TLC models passed |
| fresh Boogie installation and entry point from another directory | passed |

Tool versions were Z3 4.15.4, Boogie 3.5.6, Lean 4.28.0, TLC 1.7.4, and
Elixir 1.20.4 on OTP 29. Reports retain their own tool versions and the verifier's
source fingerprint. Run artifacts and the complete local check log are in `.vex`;
this directory is intentionally ignored by git.

Negative regressions include false contracts, arithmetic errors, incomplete
matches, failed callees, nonterminating ghost definitions, circular unfolding,
erased runtime calls, extra Lean axioms, `sorry`, missing tools, source-path
command injection, module-name ambiguity, invalid source exports, a lost-update
race and an unfair mailbox schedule. These must fail verification while passing
the regression tests. The fair mailbox model checks safety and eventual delivery;
the same delivery property fails without receiver fairness.

The paper regressions additionally cover clause dispatch independent of requires,
omitted clauses, symbolic tuple bounds, nested and repeated-variable patterns,
quantifier scope and definedness, tuple extensionality, lexicographic measures,
local proof state, admission propagation and runtime erasure. The shared
substitution test checks that replacing a free variable cannot capture it under
a quantifier. See [coverage](coverage.md) for the source review.

Supplemental real-tool checks exercised a multi-clause TLA+ source export using
`tuple_size` and `elem`, including evaluation of the shared recursive sequence
operators by TLC. A symbolic tuple-field size bound passed Z3 and Boogie
automatically and Lean with a supplied proof using `kernel.nth_mass`; the default
Lean tactic left that obligation unproved.

The Linux GitHub workflow invokes the same scripts. Its remote execution was not
performed in this local session. The YAML and shell scripts were checked locally.

The integration suite exercises component boundaries by executing the CLI as a
subprocess, since static call graphs do not connect Python tests to the Rust binary.
Ripwire's test gate therefore reports apparently uncovered CLI paths even though
the real-tool suite executes them. Its quality delta is not clean: remaining
warnings include expanded language dispatch/schema definitions, small constructor
helpers, repeated test shapes, and macro/test registration it does not resolve.
The added nesting in symbolic sequence evaluation was removed. The remaining
warnings were reviewed as explicit trade-offs; neither static gate is reported
as passed.

This validation covers the documented discrete source profile and explicit finite
models. It does not establish a translator soundness metatheorem, arbitrary OTP
support, or automatic refinement from concurrent Elixir to TLA+. See [trust](trust.md).
