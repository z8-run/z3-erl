# proofs

## generated obligations

Run `bin/vex emit path/to/source.ex --engine lean`. Open `plan.json` to locate a
condition by owner, kind and source span, then open `vc/<id>.lean`. The theorem
contains the exact universally quantified inputs, function symbols, hypotheses
and conclusion checked by other backends. Original names map injectively to `v`
followed by their UTF-8 hexadecimal bytes; `n` becomes `v6e`.

The default `vex_auto` tactic in `kernel/lean/auto.lean` decomposes logical
summaries, then uses linear integer arithmetic (`omega`), simplification and
case splitting. It does not import Mathlib or replay a claimed SMT result.
Unresolved goals remain unproved.

## supply a proof

Create `proofs/<id>.lean` containing only a proof term, for example:

```lean
by
  classical
  simp_all
  omega
```

Then run `bin/vex check path/to/source.ex --engine lean --proofs proofs`.
The file is inserted after the generated theorem's `:=`; it cannot replace the
generated statement. Local Lean source is trusted developer code, not sandboxed
data. The resulting declaration is audited for prohibited axioms.

Definitions and foundational lemmas are in `kernel/lean`. Run
`lake +leanprover/lean4:v4.28.0 build` after editing them. The Lean backend also
builds this library once before checking a batch, so stale `.olean` files are not
used as a substitute for current sources. `bin/vex kernel` regenerates
`kernel/lean/term.lean` from the common datatype description; a Rust test checks
that the committed file matches.

## scope

Discharging every VC is the tool's evidence about a source contract under its
translation assumptions. The metatheorem that its Rust VC generator implements
the Hoare rules has not yet been proved. Unbounded actor invariants additionally
need an operational model and a refinement argument; a TLC success alone does
not supply either theorem.
