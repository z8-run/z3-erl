defmodule Wrong do
  use :vex
  @verifier requires is_integer(n)
  @verifier ensures inc(n) === n + 2
  defv inc(n), do: n + 1
end
