defmodule Paper do
  use :vex

  @verifier requires is_tuple(t) and is_integer(i) and i >= 0 and i < tuple_size(t)
  @verifier ensures fetch(t, i) === elem(t, i)
  defv(fetch(t, i), do: elem(t, i))

  @verifier ensures empty(t) === {}
  defv(empty(t) when is_tuple(t) and tuple_size(t) === 0, do: t)

  @verifier ensures pair({a, b}) === {b, a}
  defv(pair({a, b}), do: {b, a})

  @verifier ensures first([h | t]) === h
  defv(first([h | t]), do: h)

  @verifier ensures classify(x) === :number
  defv(classify(x) when is_integer(x), do: :number)

  @verifier ensures classify(x) === :other
  defv(classify(x), do: :other)

  @verifier ensures dispatch() === {:number, :other}
  defv(dispatch(), do: {classify(3), classify([])})

  @verifier ensures guard(x) === :head
  defv(guard(x) when hd(x) === 1, do: :head)

  @verifier ensures guard(x) === :other
  defv(guard(x), do: :other)

  @verifier ensures fallback() === :other
  defv(fallback(), do: guard([]))

  @verifier ensures nested() === {1, [2, 3]}
  defv nested() do
    [h | t = [_, 3]] = [1, 2, 3]
    {h, t}
  end

  @verifier ensures is_integer(cells(xs)) and cells(xs) >= 1
  @verifier decreases term_size(xs)
  defvg(cells([_ | t] = xs), do: 1 + cells(t))

  @verifier ensures cells(xs) === 0
  @verifier decreases term_size(xs)
  defvg(cells(xs), do: 0)

  @verifier ensures proof() === 7
  defv proof() do
    ghost do
      havoc(x)
      assert x === x
      assert forall(y, do: y === y)
      assert exists(y, do: y === x)

      block do
        havoc(x)
        assert x === x
      end
    end

    7

    ghost do
      unfold cells([1, 2 | false])
      assert cells([1, 2 | false]) >= 1
    end
  end

  @verifier requires is_integer(x)
  defv(dup(x), do: x + x)

  @verifier ensures uses_dup(y) === 2 * y
  defv uses_dup(y) when is_integer(y) do
    unfold dup(y)
    dup(y)
  end

  @verifier ensures literals() === true
  defv literals() do
    assert 4 - 2 === 6 - 4
    assert (false or 2) === 2
    assert 3 > 2 and 1 <= 1
    assert elem({1, 2, 3}, 0) === 1
    assert [1 | [2 | [3 | []]]] === [1, 2, 3]
    assert true or true + true
    is_list([1 | false]) and is_list([]) and not is_list(nil)
  end
end
