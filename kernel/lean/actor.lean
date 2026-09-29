import kernel.lean.audit

namespace kernel

/-- Reachability permits every interleaving admitted by the supplied step relation. -/
inductive reaches {state : Type} (step : state → state → Prop) (start : state) : state → Prop
  | initial : reaches step start start
  | next {before after : state} : reaches step start before → step before after → reaches step start after

theorem invariant {state : Type} (step : state → state → Prop) (inv : state → Prop)
    (start : state) (initial : inv start)
    (preserved : ∀ before after, inv before → step before after → inv after) :
    ∀ s, reaches step start s → inv s := by
  intro s run
  induction run with
  | initial => exact initial
  | next _ hstep ih => exact preserved _ _ ih hstep

/-- A refinement must connect every concrete step; naming a model is insufficient. -/
theorem simulation {concrete abstract : Type}
    (cstep : concrete → concrete → Prop) (astep : abstract → abstract → Prop)
    (view : concrete → abstract)
    (refines : ∀ a b, cstep a b → astep (view a) (view b))
    (start finish : concrete) (run : reaches cstep start finish) :
    reaches astep (view start) (view finish) := by
  induction run with
  | initial => exact .initial
  | next _ hstep ih => exact .next ih (refines _ _ hstep)

#vex_audit invariant
#vex_audit simulation
end kernel
