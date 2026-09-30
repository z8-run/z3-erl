import kernel.lean.term
import kernel.lean.audit

namespace kernel

@[simp] theorem len_nonneg (xs : seq) : 0 ≤ len xs := by
  cases xs with
  | empty => simp
  | push x xs => have h := len_nonneg xs; simp only [len]; omega
termination_by sizeOf xs

@[simp] theorem len_zero (xs : seq) : len xs = 0 ↔ xs = .empty := by
  cases xs with
  | empty => simp
  | push h t =>
    constructor
    · intro eq
      have ht := len_nonneg t
      simp only [len] at eq
      omega
    · intro eq
      cases eq

mutual
@[simp] theorem size_pos (x : term) : 1 ≤ size x := by
  cases x with
  | int n => simp
  | atom n => simp
  | nil => simp
  | cons h t =>
    have hh := size_pos h
    have ht := size_pos t
    simp only [size]
    omega
  | tuple xs =>
    have h := mass_nonneg xs
    simp only [size]
    omega
termination_by sizeOf x

@[simp] theorem mass_nonneg (xs : seq) : 0 ≤ mass xs := by
  cases xs with
  | empty => simp
  | push h t =>
    have hh := size_pos h
    have ht := mass_nonneg t
    simp only [mass]
    omega
termination_by sizeOf xs
end

theorem nth_mass (xs : seq) (i : Int) (lo : 0 ≤ i) (hi : i < len xs) :
    size (nth xs i) ≤ mass xs := by
  cases xs with
  | empty => simp only [len] at hi; omega
  | push h t =>
    by_cases zero : i = 0
    · subst i
      have hm := mass_nonneg t
      simp only [nth, mass, Int.le_refl, ↓reduceIte]
      omega
    · have ih := nth_mass t (i-1) (by omega) (by simp only [len] at hi; omega)
      have hh := size_pos h
      simp only [nth, lo, zero, ↓reduceIte, mass]
      omega
termination_by sizeOf xs

theorem seq_ext (xs ys : seq) (length : len xs = len ys)
    (fields : ∀ i : Int, 0 ≤ i → i < len xs → nth xs i = nth ys i) : xs = ys := by
  cases xs with
  | empty =>
    cases ys with
    | empty => rfl
    | push h t => have ht := len_nonneg t; simp only [len] at length; omega
  | push h t =>
    cases ys with
    | empty => have ht := len_nonneg t; simp only [len] at length; omega
    | push k u =>
      have ht := len_nonneg t
      have heads := fields 0 (by omega) (by simp only [len]; omega)
      have tails := seq_ext t u (by simp only [len] at length; omega) (by
        intro i lo hi
        have h := fields (i+1) (by omega) (by simp only [len]; omega)
        simpa only [nth, show 0 ≤ i+1 by omega, show i+1 ≠ 0 by omega,
          ↓reduceIte, Int.add_sub_cancel] using h)
      simp only [nth, Int.le_refl, ↓reduceIte] at heads
      cases heads
      cases tails
      rfl
termination_by sizeOf xs

#vex_audit len_zero
#vex_audit len_nonneg
#vex_audit size_pos
#vex_audit mass_nonneg
#vex_audit nth_mass
#vex_audit seq_ext

/-- The disjunctive form emitted by guarded Elixir quantifiers. -/
theorem seq_from_fields (xs ys : seq) (length : len xs = len ys)
    (fields : ∀ i : Int, (i < 0 ∨ len ys ≤ i) ∨ nth xs i = nth ys i) : xs = ys := by
  apply seq_ext xs ys length
  intro i lo hi
  rcases fields i with (negative | outside) | equal
  · omega
  · omega
  · exact equal

#vex_audit seq_from_fields

theorem all_int (p : term → Prop) :
    (∀ x, ¬is_int x ∨ p x) ↔ ∀ i, p (.int i) := by
  constructor
  · intro h i
    simpa using h (.int i)
  · intro h x
    cases x <;> simp_all

theorem exists_int (p : term → Prop) :
    (∃ x, is_int x ∧ p x) ↔ ∃ i, p (.int i) := by
  constructor
  · rintro ⟨x, hx, hp⟩
    cases x <;> simp_all
    exact ⟨_, hp⟩
  · rintro ⟨i, hi⟩
    exact ⟨.int i, by simp, hi⟩

#vex_audit all_int
#vex_audit exists_int
end kernel
