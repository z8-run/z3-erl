import Lean
import Lean.Elab.Tactic.Omega

open Lean Elab Command in
elab "#vex_audit " n:ident : command => do
  let name ← liftCoreM <| realizeGlobalConstNoOverloadWithInfo n
  let axioms ← Lean.collectAxioms name
  for ax in axioms do
    unless #[`propext, `Classical.choice, `Quot.sound].contains ax do
      throwError "vex rejected axiom {ax} in {name}"
  logInfo m!"vex audit ok: {name}"
