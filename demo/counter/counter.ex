defmodule Counter do
  use :vex

  @verifier requires is_integer(state) and is_integer(delta) and state >= 0
  @verifier ensures step(state, delta) >= 0
  defv step(state, delta) do
    next = state + delta
    if next < 0, do: 0, else: next
  end
end
