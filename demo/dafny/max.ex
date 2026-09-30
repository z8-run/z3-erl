# Adapted from Dafny maximum.dfy; see readme.md and license.
defmodule :max do
  use :vex

  @verifier requires is_tuple(xs) and tuple_size(xs) > 0
  @verifier requires forall(i,
                       do:
                         not is_integer(i) or i < 0 or i >= tuple_size(xs) or
                           is_integer(elem(xs, i))
                     )
  @verifier ensures is_integer(find(xs))
  @verifier ensures exists(i,
                      do:
                        is_integer(i) and i >= 0 and i < tuple_size(xs) and
                          elem(xs, i) === find(xs)
                    )
  @verifier ensures forall(i,
                      do:
                        not is_integer(i) or i < 0 or i >= tuple_size(xs) or
                          elem(xs, i) <= find(xs)
                    )
  defv find(xs) do
    scan(xs, 0, elem(xs, 0))
  end

  @verifier requires is_tuple(xs) and is_integer(i) and is_integer(m)
  @verifier requires 0 <= i and i <= tuple_size(xs)
  @verifier requires forall(j,
                       do:
                         not is_integer(j) or j < 0 or j >= tuple_size(xs) or
                           is_integer(elem(xs, j))
                     )
  @verifier requires exists(j,
                       do: is_integer(j) and j >= 0 and j < tuple_size(xs) and elem(xs, j) === m
                     )
  @verifier requires forall(j, do: not is_integer(j) or j < 0 or j >= i or elem(xs, j) <= m)
  @verifier ensures is_integer(scan(xs, i, m))
  @verifier ensures exists(j,
                      do:
                        is_integer(j) and j >= 0 and j < tuple_size(xs) and
                          elem(xs, j) === scan(xs, i, m)
                    )
  @verifier ensures forall(j,
                      do:
                        not is_integer(j) or j < 0 or j >= tuple_size(xs) or
                          elem(xs, j) <= scan(xs, i, m)
                    )
  @verifier decreases tuple_size(xs) - i
  defvp scan(xs, i, m) do
    if i === tuple_size(xs) do
      m
    else
      x = elem(xs, i)
      if m < x, do: scan(xs, i + 1, x), else: scan(xs, i + 1, m)
    end
  end

  @verifier requires is_tuple(xs) and is_integer(a) and is_integer(b)
  @verifier requires forall(i,
                       do:
                         not is_integer(i) or i < 0 or i >= tuple_size(xs) or
                           is_integer(elem(xs, i))
                     )
  @verifier requires exists(i,
                       do: is_integer(i) and i >= 0 and i < tuple_size(xs) and elem(xs, i) === a
                     )
  @verifier requires exists(i,
                       do: is_integer(i) and i >= 0 and i < tuple_size(xs) and elem(xs, i) === b
                     )
  @verifier requires forall(i,
                       do: not is_integer(i) or i < 0 or i >= tuple_size(xs) or elem(xs, i) <= a
                     )
  @verifier requires forall(i,
                       do: not is_integer(i) or i < 0 or i >= tuple_size(xs) or elem(xs, i) <= b
                     )
  @verifier ensures a === b
  defvg(unique(xs, a, b), do: nil)
end
