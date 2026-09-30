# Adapted from DafnyAST.interpExpr; see readme.md and license.
defmodule :expr do
  use :vex

  @verifier ensures is_boolean(valid(e))
  @verifier ensures valid(e) !== true or
                      (is_tuple(e) and
                         ((tuple_size(e) === 2 and (elem(e, 0) === :const or elem(e, 0) === :var)) or
                            (tuple_size(e) === 3 and (elem(e, 0) === :add or elem(e, 0) === :sub))))
  @verifier decreases term_size(e)
  defvg valid(e) do
    case e do
      {:const, n} -> is_integer(n)
      {:var, i} -> is_integer(i)
      {:add, a, b} -> valid(a) and valid(b)
      {:sub, a, b} -> valid(a) and valid(b)
      _ -> false
    end
  end

  @verifier requires valid(e) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures is_integer(value(e, ctx))
  @verifier decreases term_size(e)
  defvg value(e, ctx) do
    valid(e)

    case e do
      {:const, n} ->
        unfold valid({:const, n})
        n

      {:var, i} ->
        unfold valid({:var, i})
        :env.get(ctx, i)

      {:add, a, b} ->
        unfold valid({:add, a, b})
        value(a, ctx) + value(b, ctx)

      {:sub, a, b} ->
        unfold valid({:sub, a, b})
        value(a, ctx) - value(b, ctx)
    end
  end

  @verifier requires valid(e) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures is_integer(eval(e, ctx)) and eval(e, ctx) === value(e, ctx)
  @verifier decreases term_size(e)
  defv eval(e, ctx) do
    ghost do
      valid(e)
    end

    case e do
      {:const, n} ->
        unfold value({:const, n}, ctx)
        n

      {:var, i} ->
        unfold value({:var, i}, ctx)
        :env.get(ctx, i)

      {:add, a, b} ->
        unfold value({:add, a, b}, ctx)
        eval(a, ctx) + eval(b, ctx)

      {:sub, a, b} ->
        unfold value({:sub, a, b}, ctx)
        eval(a, ctx) - eval(b, ctx)
    end
  end
end
