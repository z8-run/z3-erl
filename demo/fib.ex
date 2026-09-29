defmodule Fib do
  use :vex

  @verifier requires is_integer(n)
  @verifier ensures is_integer(fib(n))
  defvg fib(n) when n >= 0 do
    case n do
      0 -> 0
      1 -> 1
      n -> fib(n - 1) + fib(n - 2)
    end
  end

  @verifier requires is_integer(i) and is_integer(n) and
                       is_integer(a) and is_integer(b) and
                       i >= 0 and n >= i and
                       a === fib(i) and b === fib(i + 1)
  @verifier ensures is_integer(aux(n, i, a, b)) and aux(n, i, a, b) === fib(n)
  @verifier decreases n - i
  defvp aux(n, i, a, b) do
    if n === i do
      a
    else
      unfold fib(i + 2)
      ghost do
        assert i + 2 - 2 === i
        assert i + 2 - 1 === i + 1
        assert i + 1 + 1 === i + 2
      end
      aux(n, i + 1, b, a + b)
    end
  end

  @verifier requires is_integer(n)
  @verifier ensures compute(n) === fib(n)
  defv compute(n) when n >= 0 do
    unfold fib(0)
    unfold fib(1)
    aux(n, 0, 0, 1)
  end
end
