import kernel.lean.audit
import kernel.lean.seq

open Lean Elab Tactic Meta in
elab "vex_split" : tactic => withMainContext do
  for decl in (← getLCtx) do
    if decl.isImplementationDetail then continue
    if (← whnf decl.type).isAppOfArity ``And 2 then
      let goals ← (← getMainGoal).cases decl.fvarId
      replaceMainGoal (goals.toList.map (·.mvarId))
      return
  throwError "no conjunctive hypothesis"

open Lean Elab Tactic Meta in
elab "vex_bounds" : tactic => withMainContext do
  let mut proofs := #[]
  for decl in (← getLCtx) do
    if decl.isImplementationDetail then continue
    let type ← whnf decl.type
    if type.isConstOf ``kernel.term then
      proofs := proofs.push (← mkAppM ``kernel.size_pos #[decl.toExpr])
    if type.isConstOf ``kernel.seq then
      proofs := proofs.push (← mkAppM ``kernel.mass_nonneg #[decl.toExpr])
      proofs := proofs.push (← mkAppM ``kernel.len_nonneg #[decl.toExpr])
  let mut goal ← getMainGoal
  for proof in proofs do
    let next ← goal.assert (← mkFreshUserName `bound) (← inferType proof) proof
    let (_, introduced) ← next.intro1
    goal := introduced
  replaceMainGoal [goal]

-- Decompose logical summaries before rewriting recursive function equalities.
-- Every branch still produces an ordinary Lean proof, checked by #vex_audit.
macro "vex_auto" : tactic => `(tactic|
  (classical
   repeat' vex_split
   subst_vars
   first
   | solve
     | repeat' first
       | assumption
       | (apply And.intro)
       | omega
   | (simp_all
      repeat' first
        | (intro)
        | (apply kernel.seq_from_fields <;> assumption)
        | (vex_bounds; omega)
        | (split at * <;> simp_all))))
