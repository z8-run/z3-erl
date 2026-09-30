# Compiler register lookup, with integer variable ids; see readme.md and license.
defmodule :env do
  use :vex

  @verifier ensures is_boolean(valid(ctx))
  @verifier ensures valid(ctx) !== true or is_tuple(ctx)
  defvg valid(ctx) do
    if is_tuple(ctx) do
      forall(i,
        do: not is_integer(i) or i < 0 or i >= tuple_size(ctx) or is_integer(elem(ctx, i))
      )
    else
      false
    end
  end

  @verifier requires is_tuple(ctx) and valid(ctx) === true and is_integer(i)
  @verifier ensures is_integer(get(ctx, i))
  @verifier ensures get(ctx, i) === if(i >= 0 and i < tuple_size(ctx), do: elem(ctx, i), else: 0)
  defv get(ctx, i) do
    unfold valid(ctx)
    if i >= 0 and i < tuple_size(ctx), do: elem(ctx, i), else: 0
  end
end
