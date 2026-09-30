# Fixed Parentheses combinators, made first order; see readme.md and license.
defmodule :parse do
  use :vex

  @verifier requires is_tuple(xs) and is_integer(i) and is_integer(n) and
                       i >= 0 and n >= 0 and i + 2 * n <= tuple_size(xs)
  @verifier ensures is_boolean(good(xs, i, n))
  defvg good(xs, i, n) do
    forall(k,
      do:
        not is_integer(k) or k < i or k >= i + 2 * n or
          elem(xs, k) === if(k < i + n, do: 40, else: 41)
    )
  end

  @verifier requires is_tuple(xs) and is_integer(i) and is_integer(c) and
                       i >= 0 and i <= tuple_size(xs)
  @verifier ensures char(xs, i, c) ===
                      if(i < tuple_size(xs) and elem(xs, i) === c, do: i + 1, else: :error)
  @verifier ensures char(xs, i, c) === :error or
                      (is_integer(char(xs, i, c)) and char(xs, i, c) === i + 1 and
                         char(xs, i, c) <= tuple_size(xs))
  defv char(xs, i, c) do
    if i < tuple_size(xs) and elem(xs, i) === c, do: i + 1, else: :error
  end

  @verifier requires is_tuple(xs) and is_integer(i) and is_integer(n) and
                       i >= 0 and n >= 0 and i + 2 * (n + 1) <= tuple_size(xs)
  @verifier requires elem(xs, i) === 40 and elem(xs, i + 2 * n + 1) === 41 and
                       good(xs, i + 1, n) === true
  @verifier ensures good(xs, i, n + 1) === true
  defvg wrap(xs, i, n) do
    unfold good(xs, i + 1, n)
    unfold good(xs, i, n + 1)
  end

  @verifier requires is_tuple(xs) and is_integer(i) and i >= 0 and i <= tuple_size(xs)
  @verifier ensures is_tuple(walk(xs, i)) and tuple_size(walk(xs, i)) === 2 and
                      is_integer(elem(walk(xs, i), 0)) and is_integer(elem(walk(xs, i), 1)) and
                      elem(walk(xs, i), 0) >= i and elem(walk(xs, i), 0) <= tuple_size(xs) and
                      elem(walk(xs, i), 1) >= 0 and
                      elem(walk(xs, i), 0) === i + 2 * elem(walk(xs, i), 1)
  @verifier ensures good(xs, i, elem(walk(xs, i), 1)) === true
  @verifier decreases tuple_size(xs) - i
  defvp walk(xs, i) do
    case char(xs, i, 40) do
      :error ->
        unfold good(xs, i, 0)
        {i, 0}

      j ->
        {k, n} = walk(xs, j)

        case char(xs, k, 41) do
          :error ->
            unfold good(xs, i, 0)
            {i, 0}

          last ->
            ghost do
              wrap(xs, i, n)
            end

            {last, n + 1}
        end
    end
  end

  @verifier requires is_tuple(xs)
  @verifier ensures run(xs) === :error or
                      (is_integer(run(xs)) and run(xs) >= 0 and tuple_size(xs) === 2 * run(xs) and
                         good(xs, 0, run(xs)) === true)
  defv run(xs) do
    {last, n} = walk(xs, 0)
    if last === tuple_size(xs), do: n, else: :error
  end
end
