import kernel.lean.term
import kernel.lean.audit

namespace kernel

/-- Successful-return partial correctness. Termination is a separate proposition. -/
def hoare (run : term → term → Prop) (pre : term → Prop)
    (post : term → term → Prop) : Prop :=
  ∀ input output, pre input → run input output → post input output

def total (run : term → term → Prop) (pre : term → Prop)
    (post : term → term → Prop) : Prop :=
  hoare run pre post ∧ ∀ input, pre input → ∃ output, run input output

theorem strengthen (run : term → term → Prop) (pre stronger : term → Prop)
    (post : term → term → Prop) (h : hoare run pre post)
    (entails : ∀ x, stronger x → pre x) : hoare run stronger post := by
  intro input output hp hr
  exact h input output (entails input hp) hr

/-- Composition does not assume the second computation terminates. -/
theorem compose (first second : term → term → Prop) (pre mid : term → Prop)
    (post : term → Prop)
    (hfirst : hoare first pre (fun _ output => mid output))
    (hsecond : hoare second mid (fun _ output => post output)) :
    hoare (fun input output => ∃ middle, first input middle ∧ second middle output)
      pre (fun _ output => post output) := by
  intro input output hp hr
  obtain ⟨middle, h1, h2⟩ := hr
  exact hsecond middle output (hfirst input middle hp h1) h2

theorem integer_injective (a b : Int) : term.int a = term.int b ↔ a = b := by
  simp

theorem nil_is_not_empty : term.atom 2 ≠ term.nil := by
  simp

def trunc (a b : Int) : Int :=
  let q := a.natAbs / b.natAbs
  if (a < 0) = (b < 0) then Int.ofNat q else -Int.ofNat q

def rem (a b : Int) : Int := a - trunc a b * b

theorem division_identity (a b : Int) : trunc a b * b + rem a b = a := by
  simp only [rem]
  omega

example : trunc (-7) 3 = -2 ∧ rem (-7) 3 = -1 := by decide
example : trunc 7 (-3) = -2 ∧ rem 7 (-3) = 1 := by decide

#vex_audit strengthen
#vex_audit compose
#vex_audit integer_injective
#vex_audit nil_is_not_empty
#vex_audit division_identity
end kernel
