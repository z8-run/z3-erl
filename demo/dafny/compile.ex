# Adapted from Compiler.compileExpr and compileExprCorrect'; see readme.md and license.
defmodule :compiler do
  use :vex

  @verifier requires :expr.valid(e) === true
  @verifier ensures :stack.valid(code(e)) === true and :seq.proper(code(e)) === true
  @verifier ensures code(e) ===
                      if(tuple_size(e) === 2,
                        do: [e],
                        else: [elem(e, 0) | :seq.cat(code(elem(e, 2)), code(elem(e, 1)))]
                      )
  @verifier decreases term_size(e)
  defvg code(e) do
    :expr.valid(e)

    case e do
      {:const, n} ->
        unfold :expr.valid({:const, n})
        unfold :stack.instr({:const, n})
        unfold :stack.valid([])
        unfold :stack.valid([{:const, n}])
        [{:const, n}]

      {:var, i} ->
        unfold :expr.valid({:var, i})
        unfold :stack.instr({:var, i})
        unfold :stack.valid([])
        unfold :stack.valid([{:var, i}])
        [{:var, i}]

      {:add, a, b} ->
        unfold :expr.valid({:add, a, b})
        x = code(a)
        y = code(b)
        p = :stack.join(y, x)
        unfold :stack.instr(:add)
        unfold :stack.valid([:add | p])
        [:add | p]

      {:sub, a, b} ->
        unfold :expr.valid({:sub, a, b})
        x = code(a)
        y = code(b)
        p = :stack.join(y, x)
        unfold :stack.instr(:sub)
        unfold :stack.valid([:sub | p])
        [:sub | p]
    end
  end

  @verifier requires :expr.valid(e) === true
  @verifier ensures :stack.valid(run(e)) === true and :seq.proper(run(e)) === true
  @verifier ensures run(e) === code(e)
  @verifier decreases term_size(e)
  defv run(e) do
    ghost do
      :expr.valid(e)
    end

    case e do
      {:const, n} ->
        ghost do
          code({:const, n})
        end

        [{:const, n}]

      {:var, i} ->
        ghost do
          code({:var, i})
        end

        [{:var, i}]

      {:add, a, b} ->
        ghost do
          code({:add, a, b})
        end

        unfold :expr.valid({:add, a, b})
        [:add | :seq.append(run(b), run(a))]

      {:sub, a, b} ->
        ghost do
          code({:sub, a, b})
        end

        unfold :expr.valid({:sub, a, b})
        [:sub | :seq.append(run(b), run(a))]
    end
  end

  @verifier requires is_integer(n) and :stack.ints(s) === true
  @verifier ensures :stack.ints([n | s]) === true
  defvg push(n, s) do
    unfold :stack.ints([n | s])
  end

  @verifier requires (op === :add or op === :sub) and is_integer(a) and is_integer(b) and
                       :stack.ints(s) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures calc(op, a, b, s, ctx) === :stack.op(op, [b, a | s], ctx)
  @verifier ensures calc(op, a, b, s, ctx) ===
                      if(op === :add, do: [a + b | s], else: [a - b | s])
  defvg calc(op, a, b, s, ctx) do
    push(a, s)
    push(b, [a | s])

    if op === :add do
      unfold :stack.op(:add, [b, a | s], ctx)
      [a + b | s]
    else
      unfold :stack.op(:sub, [b, a | s], ctx)
      [a - b | s]
    end
  end

  @verifier requires :expr.valid(e) === true and :stack.ints(s) === true and
                       is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures :stack.run(code(e), s, ctx) === [:expr.value(e, ctx) | s]
  @verifier decreases term_size(e)
  defvg correct(e, s, ctx) do
    :expr.valid(e)
    :expr.value(e, ctx)

    case e do
      {:const, n} ->
        code({:const, n})
        unfold :expr.value({:const, n}, ctx)
        unfold :stack.run([{:const, n}], s, ctx)
        unfold :stack.run([], s, ctx)
        unfold :stack.op({:const, n}, s, ctx)

      {:var, i} ->
        code({:var, i})
        unfold :expr.value({:var, i}, ctx)
        unfold :stack.run([{:var, i}], s, ctx)
        unfold :stack.run([], s, ctx)
        unfold :stack.op({:var, i}, s, ctx)

      {:add, a, b} ->
        code({:add, a, b})
        unfold :expr.value({:add, a, b}, ctx)
        x = code(a)
        y = code(b)
        p = :stack.join(y, x)
        av = :expr.value(a, ctx)
        bv = :expr.value(b, ctx)
        correct(a, s, ctx)
        push(av, s)
        correct(b, [av | s], ctx)
        :stack.concat(y, x, s, ctx)
        unfold :stack.run([:add | p], s, ctx)
        calc(:add, av, bv, s, ctx)

      {:sub, a, b} ->
        code({:sub, a, b})
        unfold :expr.value({:sub, a, b}, ctx)
        x = code(a)
        y = code(b)
        p = :stack.join(y, x)
        av = :expr.value(a, ctx)
        bv = :expr.value(b, ctx)
        correct(a, s, ctx)
        push(av, s)
        correct(b, [av | s], ctx)
        :stack.concat(y, x, s, ctx)
        unfold :stack.run([:sub | p], s, ctx)
        calc(:sub, av, bv, s, ctx)
    end
  end
end
