defmodule :vex do
  @moduledoc """
  Contracts for vex. Checking is explicit and separate from application compilation.

      use :vex
      @verifier requires is_integer(n)
      @verifier ensures inc(n) === n + 1
      defv inc(n), do: n + 1

  Ghost code has no runtime effects. `assert` and `unfold` are proof statements.
  """

  defmacro __using__(_) do
    quote do
      import :vex
      Module.register_attribute(__MODULE__, :verifier, accumulate: true)
    end
  end

  for name <- [:requires, :ensures, :decreases] do
    defmacro unquote(name)(expr) do
      Macro.escape({unquote(name), expr})
    end
  end

  defmacro defv(head, do: body), do: define(:def, head, body)
  defmacro defvp(head, do: body), do: define(:defp, head, body)

  defmacro defvg(_head, do: _body) do
    quote do
      Module.delete_attribute(__MODULE__, :verifier)
    end
  end

  defmacro ghost(do: _body), do: nil
  defmacro assert(_expr), do: nil
  defmacro unfold(_expr), do: nil

  defp define(kind, head, body) do
    clean = erase(body)

    quote do
      Module.delete_attribute(__MODULE__, :verifier)
      unquote(kind)(unquote(head), do: unquote(clean))
    end
  end

  defp erase(ast) do
    Macro.prewalk(ast, fn
      {name, _, _} when name in [:ghost, :assert, :unfold] -> nil
      other -> other
    end)
  end
end
