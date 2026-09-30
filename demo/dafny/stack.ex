# Expression instructions from StackMachine; see readme.md and license.
defmodule :stack do
  use :vex

  @verifier ensures is_boolean(ints(s))
  @verifier ensures ints(s) !== true or is_list(s)
  @verifier decreases term_size(s)
  defvg ints(s) do
    case s do
      [] -> true
      [h | t] -> is_integer(h) and ints(t)
      _ -> false
    end
  end

  @verifier ensures is_boolean(instr(i))
  defvg instr(i) do
    case i do
      {:const, n} -> is_integer(n)
      {:var, n} -> is_integer(n)
      :add -> true
      :sub -> true
      _ -> false
    end
  end

  @verifier ensures is_boolean(valid(p))
  @verifier ensures valid(p) !== true or (is_list(p) and :seq.proper(p) === true)
  @verifier decreases term_size(p)
  defvg valid(p) do
    case p do
      [] ->
        unfold :seq.proper([])
        true

      [h | t] ->
        a = instr(h)
        b = valid(t)
        unfold :seq.proper([h | t])
        a and b

      _ ->
        false
    end
  end

  @verifier requires ints(s) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures ints(op(i, s, ctx)) === true
  defvg op(i, s, ctx) do
    case {i, s} do
      {{:const, n}, _} when is_integer(n) ->
        unfold ints([n | s])
        [n | s]

      {{:var, n}, _} when is_integer(n) ->
        v = :env.get(ctx, n)
        unfold ints([v | s])
        [v | s]

      {:add, [b, a | t]} ->
        unfold ints([b, a | t])
        unfold ints([a | t])
        unfold ints([a + b | t])
        [a + b | t]

      {:sub, [b, a | t]} ->
        unfold ints([b, a | t])
        unfold ints([a | t])
        unfold ints([a - b | t])
        [a - b | t]

      _ ->
        s
    end
  end

  @verifier requires ints(s) === true and is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures ints(step(i, s, ctx)) === true and step(i, s, ctx) === op(i, s, ctx)
  defv step(i, s, ctx) do
    case {i, s} do
      {{:const, n}, _} when is_integer(n) ->
        unfold op({:const, n}, s, ctx)
        [n | s]

      {{:var, n}, _} when is_integer(n) ->
        unfold op({:var, n}, s, ctx)
        [:env.get(ctx, n) | s]

      {:add, [b, a | t]} ->
        unfold op(:add, [b, a | t], ctx)
        [a + b | t]

      {:sub, [b, a | t]} ->
        unfold op(:sub, [b, a | t], ctx)
        [a - b | t]

      _ ->
        unfold op(i, s, ctx)
        s
    end
  end

  @verifier requires valid(p) === true and ints(s) === true and
                       is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures ints(run(p, s, ctx)) === true
  @verifier decreases term_size(p)
  defvg run(p, s, ctx) do
    valid(p)

    case p do
      [] ->
        s

      [h | t] ->
        unfold valid([h | t])
        op(h, run(t, s, ctx), ctx)
    end
  end

  @verifier requires valid(p) === true and ints(s) === true and
                       is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures ints(exec(p, s, ctx)) === true and exec(p, s, ctx) === run(p, s, ctx)
  @verifier decreases term_size(p)
  defv exec(p, s, ctx) do
    ghost do
      valid(p)
    end

    case p do
      [] ->
        unfold run([], s, ctx)
        s

      [h | t] ->
        unfold run([h | t], s, ctx)
        step(h, exec(t, s, ctx), ctx)
    end
  end

  @verifier requires valid(p) === true and valid(q) === true
  @verifier ensures valid(join(p, q)) === true and join(p, q) === :seq.cat(p, q)
  @verifier decreases term_size(p)
  defvg join(p, q) do
    valid(p)
    valid(q)
    r = :seq.cat(p, q)

    case p do
      [] ->
        unfold :seq.cat([], q)

      [h | t] ->
        unfold valid([h | t])
        u = join(t, q)
        unfold :seq.cat([h | t], q)
        unfold valid([h | u])
    end

    r
  end

  @verifier requires valid(p) === true and valid(q) === true and ints(s) === true and
                       is_tuple(ctx) and :env.valid(ctx) === true
  @verifier ensures run(:seq.cat(p, q), s, ctx) === run(p, run(q, s, ctx), ctx)
  @verifier decreases term_size(p)
  defvg concat(p, q, s, ctx) do
    valid(p)
    valid(q)
    join(p, q)
    v = run(q, s, ctx)

    case p do
      [] ->
        unfold :seq.cat([], q)
        unfold run([], v, ctx)

      [h | t] ->
        unfold valid([h | t])
        u = join(t, q)
        concat(t, q, s, ctx)
        unfold :seq.cat([h | t], q)
        unfold run([h | u], s, ctx)
        unfold run([h | t], v, ctx)
    end
  end
end
