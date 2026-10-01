# check

Validated on macOS on 2026-09-30 using real installed backends. The current
validation combines the full `bin/check.sh` run with focused reruns and a
separate final project check; the admission-test failure is recorded below.

| check | result |
| --- | --- |
| Rust formatting and Clippy with warnings denied | passed |
| Rust tests | 5 passed |
| Elixir formatting, compilation and DSL tests | 2 tests passed |
| Lean library, including axiom audits | passed on `leanprover/lean4:v4.28.0` |
| CLI integration and runtime conformance | 57 cases passed across the full run and focused reruns |
| Dafny regression suite | 19 tests covering proofs, runtime behavior and rejected mutations |
| combined Dafny project | 8 source files, 36 total-correctness contracts, 1,626 Z3 obligations; no admitted assumptions |
| individual Dafny translations | all passed direct Z3 and Boogie, including expression compiler correctness and quantified parser soundness |
| clause and proof demo | 14 contracts passed in Z3, Boogie and Lean |
| Fibonacci ghost, accumulator and public function | 51 obligations passed in each of Z3, Boogie and Lean; 153 results |
| OTP integer conformance | 45 signed, zero and large-integer cases checked through all three backends |
| OTP container conformance | 19 list, tuple, predicate, size, selector and equality cases checked through all three backends |
| configured demo project | 9 contracts, 69 Z3 obligations, 2 TLC models passed |
| fresh Boogie installation and entry point from another directory | passed |

The full run completed 57 integration cases in 1,332 seconds: 56 passed, and the
admission-propagation test failed its status assertion. That assertion omitted
backend evidence, so the original engine failure could not be diagnosed from the
log. An independent rerun produced the expected conditional contracts, with
obligations proved under the recorded assumption through all three engines.
Cold Lean startup had separately been observed taking 18 seconds, exceeding the
old 10-second test budget. The test harness and Makefile now allow 30 seconds for
`lean` and `all` unless a timeout is supplied explicitly, and the admission test
includes backend evidence in failed assertions. Admission propagation, basic
contracts, quantified ghost values and custom Lean proof terms passed focused
reruns. The final configured project was checked separately and passed both TLC
models. Logs are `.vex/dafny/check.log`, `recheck.log`, `lean-recheck.log`, and
`project.log`; the full run's failure remains in its log.

The Makefile was also exercised with real passing and failing checks, focused
runtime tests, and report summaries. Dry runs checked group order under `make -j`,
sample dependency selection, unknown-sample rejection and timeout overrides.
`make pass` avoids repeating the basic demo for each engine separately.

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
Ripwire's test gate therefore still reports uncovered CLI paths; that gate is not
reported as passed. Quality reviews at `b915aa796+dirty` (translations) and
`ddd7cbd07+dirty` (timeout handling) reported no gating quality regressions.
Nonblocking findings included module sizes, test discovery and similar mutation
cases. Mutation execution is shared through one helper while individual cases
remain separately selectable. Edit checks found no incompatible callers of the
symbolic evaluator or test harness.

This validation covers the documented discrete source profile and explicit finite
models. It does not establish a translator soundness metatheorem, arbitrary OTP
support, or automatic refinement from concurrent Elixir to TLA+. See [trust](trust.md).
