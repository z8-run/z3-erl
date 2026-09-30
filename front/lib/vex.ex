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
  defmacro assert(_expr, _message), do: nil
  defmacro unfold(_expr), do: nil
  defmacro assume(_expr), do: nil
  defmacro havoc(_var), do: nil
  defmacro block(do: _body), do: nil
  defmacro forall(_vars, do: _body), do: nil
  defmacro exists(_vars, do: _body), do: nil
  defmacro term_size(_expr), do: raise(ArgumentError, "term_size is a proof operation")

  defp define(kind, head, body) do
    clean = erase(body)

    quote do
      Module.delete_attribute(__MODULE__, :verifier)
      unquote(kind)(unquote(head), do: unquote(clean))
    end
  end

  defp erase(ast) do
    Macro.prewalk(ast, fn
      {:__block__, meta, items} ->
        items = Enum.reject(items, &proof?/1)
        if items == [], do: nil, else: {:__block__, meta, items}

      other ->
        if proof?(other), do: nil, else: other
    end)
  end

  defp proof?({name, _, args}) when is_list(args),
    do: name in [:ghost, :assert, :unfold, :assume, :havoc, :block]

  defp proof?(_), do: false
end
