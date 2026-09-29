import kernel.lean.audit
import kernel.lean.term

open Lean Elab Tactic Meta in
elab "vex_split" : tactic => withMainContext do
  for decl in (← getLCtx) do
    if decl.isImplementationDetail then continue
    if (← whnf decl.type).isAppOfArity ``And 2 then
      let goals ← (← getMainGoal).cases decl.fvarId
      replaceMainGoal (goals.toList.map (·.mvarId))
      return
  throwError "no conjunctive hypothesis"

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
        | omega
        | (split at * <;> simp_all))))
