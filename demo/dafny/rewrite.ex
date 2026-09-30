# Adapted from Rewriter.simplifyExpr; see readme.md and license.
defmodule :rewrite do
  use :vex

  @verifier requires (op === :add or op === :sub) and
                       :expr.valid(a) === true and :expr.valid(b) === true
  @verifier ensures :expr.valid(pick(op, a, b)) === true
  @verifier ensures pick(op, a, b) ===
                      if(b === {:const, 0},
                        do: a,
                        else: if(op === :add and a === {:const, 0}, do: b, else: {op, a, b})
                      )
  defv pick(op, a, b) do
    if b === {:const, 0} do
      a
    else
      if op === :add and a === {:const, 0} do
        b
      else
        unfold :expr.valid({op, a, b})
        {op, a, b}
      end
    end
  end

  @verifier requires :expr.valid(e) === true
  @verifier ensures :expr.valid(norm(e)) === true
  @verifier decreases term_size(e)
  defvg norm(e) do
    :expr.valid(e)

    case e do
      {:const, _} ->
        e

      {:var, _} ->
        e

      {:add, a, b} ->
        unfold :expr.valid({:add, a, b})
        pick(:add, norm(a), norm(b))

      {:sub, a, b} ->
        unfold :expr.valid({:sub, a, b})
        pick(:sub, norm(a), norm(b))
    end
  end

  @verifier requires :expr.valid(e) === true
  @verifier ensures :expr.valid(run(e)) === true and run(e) === norm(e)
  @verifier decreases term_size(e)
  defv run(e) do
    ghost do
      :expr.valid(e)
    end

    case e do
      {:const, _n} ->
        unfold norm({:const, _n})
        e

      {:var, _i} ->
        unfold norm({:var, _i})
        e

      {:add, a, b} ->
        unfold norm({:add, a, b})
        pick(:add, run(a), run(b))

      {:sub, a, b} ->
        unfold norm({:sub, a, b})
        pick(:sub, run(a), run(b))
    end
  end

  @verifier requires (op === :add or op === :sub) and
                       :expr.valid(a) === true and :expr.valid(b) === true and
                       is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures :expr.value(local(op, a, b, ctx), ctx) === :expr.value({op, a, b}, ctx)
  @verifier ensures local(op, a, b, ctx) ===
                      if(b === {:const, 0},
                        do: a,
                        else: if(op === :add and a === {:const, 0}, do: b, else: {op, a, b})
                      )
  defvg local(op, a, b, ctx) do
    unfold :expr.valid({op, a, b})
    :expr.value(a, ctx)
    :expr.value(b, ctx)
    r = pick(op, a, b)

    if op === :add do
      unfold :expr.value({:add, a, b}, ctx)
    else
      unfold :expr.value({:sub, a, b}, ctx)
    end

    if b === {:const, 0} do
      unfold :expr.value({:const, 0}, ctx)
    else
      if op === :add and a === {:const, 0} do
        unfold :expr.value({:const, 0}, ctx)
      end
    end

    r
  end

  @verifier requires :expr.valid(e) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures :expr.value(norm(e), ctx) === :expr.value(e, ctx)
  @verifier decreases term_size(e)
  defvg correct(e, ctx) do
    :expr.valid(e)
    norm(e)

    case e do
      {:const, n} ->
        unfold norm({:const, n})

      {:var, i} ->
        unfold norm({:var, i})

      {:add, a, b} ->
        unfold norm({:add, a, b})
        correct(a, ctx)
        correct(b, ctx)
        x = norm(a)
        y = norm(b)
        local(:add, x, y, ctx)
        unfold :expr.value({:add, x, y}, ctx)
        unfold :expr.value({:add, a, b}, ctx)

      {:sub, a, b} ->
        unfold norm({:sub, a, b})
        correct(a, ctx)
        correct(b, ctx)
        x = norm(a)
        y = norm(b)
        local(:sub, x, y, ctx)
        unfold :expr.value({:sub, x, y}, ctx)
        unfold :expr.value({:sub, a, b}, ctx)
    end
  end
end
