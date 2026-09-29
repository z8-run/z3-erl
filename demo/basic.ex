defmodule Basic do
  use :vex

  @verifier requires is_integer(n)
  @verifier ensures inc(n) === n + 1
  defv inc(n), do: n + 1

  @verifier requires is_integer(n)
  @verifier ensures abs(n) >= 0
  defv abs(n) do
    if n < 0, do: -n, else: n
  end

  @verifier requires is_integer(n)
  @verifier ensures twice(n) === n + 2
  defv twice(n) do
    x = inc(n)
    inc(x)
  end

  @verifier requires is_integer(a) and is_integer(b) and b !== 0
  @verifier ensures div(a, b) * b + rem(a, b) === a
  @verifier ensures quotient(a, b) === div(a, b)
  defv quotient(a, b), do: div(a, b)

  @verifier requires is_integer(n)
  @verifier ensures first(n) === n
  defv first(n) do
    case {:ok, [n, n + 1]} do
      {:ok, [head | _tail]} -> head
    end
  end
end
