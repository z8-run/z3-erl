# check

Validated on macOS on 2026-09-30. `bin/setup.sh` and `bin/check.sh`
completed successfully using real installed backends.

| check | result |
| --- | --- |
| Rust formatting and Clippy with warnings denied | passed |
| Rust tests | 4 passed |
| Elixir formatting, compilation and DSL tests | 2 tests passed |
| Lean library, including axiom audits | passed on `leanprover/lean4:v4.28.0` |
| CLI integration and runtime conformance | 24 tests passed |
| Fibonacci ghost, accumulator and public function | 53 obligations passed in each of Z3, Boogie and Lean; 159 results |
| OTP integer conformance | 45 signed, zero and large-integer cases checked through all three backends |
| configured example project | 9 contracts, 71 Z3 obligations, 2 TLC models passed |
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

The Linux GitHub workflow invokes the same scripts. Its remote execution was not
performed in this local session. The YAML and shell scripts were checked locally.

Ripwire's structural panel and contract checks were reviewed. Delta comparison
was unavailable because this new repository had no git HEAD or previous baseline.
Its static call graph does not connect Python subprocess tests to the Rust binary;
the integration suite exercises those component boundaries by executing the CLI.

This validation covers the documented discrete source profile and explicit finite
models. It does not establish a translator soundness metatheorem, arbitrary OTP
support, or automatic refinement from concurrent Elixir to TLA+. See [trust](trust.md).
