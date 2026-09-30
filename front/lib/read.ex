defmodule :vex_read do
  @moduledoc false
  @ops ~w(+ - * div rem < <= > >= === !== == != and or && || not ! is_integer is_atom is_tuple is_boolean is_nil is_list tuple_size hd tl elem)a
  @erl ~w(+ - * div rem < > >= == not is_integer is_atom is_tuple is_boolean is_list tuple_size hd tl)a

  def files(paths) do
    Enum.reduce(
      paths,
      %{version: "vex.ir.2", files: [], functions: [], skipped: [], seen: MapSet.new()},
      fn path, out ->
        text = File.read!(path)
        ast = Code.string_to_quoted!(text, file: path, columns: true)
        file = %{path: path, hash: Base.encode16(:crypto.hash(:sha256, text), case: :lower)}
        read_top(ast, path, %{out | files: [file | out.files]})
      end
    )
    |> Map.delete(:seen)
    |> Map.update!(:files, &Enum.reverse/1)
    |> Map.update!(:functions, &group(Enum.reverse(&1)))
    |> Map.update!(:skipped, &Enum.reverse/1)
    |> complete()
  end

  defp read_top({:__block__, _, items}, path, out),
    do: Enum.reduce(items, out, &read_top(&1, path, &2))

  defp read_top({:defmodule, _, [name, [do: body]]}, path, out) do
    name = module_name(name)
    if MapSet.member?(out.seen, name), do: fail(path, 1, "duplicate module #{name}")
    out = %{out | seen: MapSet.put(out.seen, name)}
    items = block(body)
    if Enum.any?(items, &verified?/1), do: Enum.each(items, &directive(&1, path))
    {out, pending} = read_module(items, name, path, out, [])
    if pending != [], do: fail(path, 1, "contract is not followed by a function")
    out
  end

  # Top-level expressions are inventoried and never evaluated.
  defp read_top(_ast, _path, out), do: out

  defp verified?({kind, _, _}) when kind in [:defv, :defvp, :defvg], do: true
  defp verified?({:@, _, [{:verifier, _, _}]}), do: true
  defp verified?(_), do: false

  defp directive({:use, _, [:vex]}, _path), do: :ok
  defp directive({kind, _, _}, _path) when kind in [:defv, :defvp, :defvg, :def, :defp], do: :ok

  defp directive({:@, meta, [{name, _, values}]}, path) do
    cond do
      name in [
        :compile,
        :before_compile,
        :after_compile,
        :after_verify,
        :on_definition,
        :derive,
        :on_load
      ] ->
        fail(path, meta[:line], "compile hooks are outside the quoted-source profile")

      name in [
        :verifier,
        :spec,
        :type,
        :typep,
        :opaque,
        :callback,
        :macrocallback,
        :optional_callbacks
      ] ->
        :ok

      not Enum.all?(values, &Macro.quoted_literal?/1) ->
        fail(path, meta[:line], "attribute evaluation is outside the quoted-source profile")

      true ->
        :ok
    end
  end

  defp directive(ast, path),
    do:
      fail(
        path,
        line(ast),
        "unsupported module directive in verified module: #{Macro.to_string(ast)}; use fully qualified calls and move runtime setup outside the pure module"
      )

  defp read_module([], _module, _path, out, pending), do: {out, pending}

  defp read_module([ast | rest], module, path, out, pending) do
    case ast do
      {:@, _, [{:verifier, _, [{name, _, [value]}]}]}
      when name in [:requires, :ensures, :decreases] ->
        read_module(rest, module, path, out, pending ++ [{name, value}])

      {:@, meta, [{:verifier, _, _}]} ->
        fail(path, meta[:line], "expected requires, ensures, or decreases")

      {kind, meta, [head, [do: body]]} when kind in [:defv, :defvp, :defvg, :def, :defp] ->
        {name, args, guard} = head(head, path)
        owner = "#{module}.#{name}/#{length(args)}"
        loc = %{file: path, line: meta[:line] || 1}

        if pending != [] or kind in [:defv, :defvp, :defvg] do
          if kind in [:def, :defp] and has_proof?(body),
            do: fail(path, loc.line, "proof statements require defv or defvp for runtime erasure")

          ranks = Keyword.get_values(pending, :decreases)

          if length(ranks) > 1,
            do: fail(path, loc.line, "only one decreases expression is allowed")

          fun = %{
            module: module,
            name: Atom.to_string(name),
            args: Enum.with_index(args, fn _, i -> "$arg#{i}" end),
            mode: mode(kind),
            clauses: [
              %{
                patterns: Enum.map(args, &expr(&1, path)),
                requires: Enum.map(Keyword.get_values(pending, :requires), &expr(&1, path)),
                ensures: Enum.map(Keyword.get_values(pending, :ensures), &expr(&1, path)),
                guard: expr(guard, path),
                decreases: Enum.find_value(ranks, &expr(&1, path)),
                body: expr(body, path),
                span: loc
              }
            ],
            span: loc
          }

          read_module(rest, module, path, %{out | functions: [fun | out.functions]}, [])
        else
          skip = %{owner: owner, span: loc, reason: "no verification contract"}
          read_module(rest, module, path, %{out | skipped: [skip | out.skipped]}, [])
        end

      {:defmodule, _, _} ->
        fail(path, line(ast), "nested modules must be placed in their own source module")

      _ ->
        read_module(rest, module, path, out, pending)
    end
  end

  defp mode(:defvg), do: "ghost"
  defp mode(kind) when kind in [:defp, :defvp], do: "private"
  defp mode(_), do: "public"

  defp head({:when, meta, [{:when, _, _}, _]}, path),
    do: fail(path, meta[:line], "multiple when guards are outside the source profile")

  defp head({:when, _, [call, guard]}, path) do
    {name, args, _} = head(call, path)
    {name, args, guard}
  end

  defp head({name, _, args}, _path) when is_atom(name) and (is_list(args) or is_nil(args)),
    do: {name, args || [], true}

  defp head(ast, path), do: fail(path, line(ast), "unsupported function head")

  defp group(functions) do
    {order, grouped} =
      Enum.reduce(functions, {[], %{}}, fn f, {order, grouped} ->
        key = {f.module, f.name, length(f.args)}

        case grouped[key] do
          nil ->
            {[key | order], Map.put(grouped, key, f)}

          old ->
            if old.mode != f.mode,
              do:
                fail(
                  f.span.file,
                  f.span.line,
                  "clauses must have the same visibility and ghost mode"
                )

            {order, Map.put(grouped, key, %{old | clauses: [hd(f.clauses) | old.clauses]})}
        end
      end)

    Enum.map(Enum.reverse(order), fn key ->
      f = grouped[key]
      %{f | clauses: Enum.reverse(f.clauses)}
    end)
  end

  defp complete(out) do
    owners = MapSet.new(out.functions, fn f -> "#{f.module}.#{f.name}/#{length(f.args)}" end)

    for skip <- out.skipped, MapSet.member?(owners, skip.owner) do
      fail(
        skip.span.file,
        skip.span.line,
        "every clause of a selected function must be included; use defv or annotate this clause"
      )
    end

    out
  end

  defp has_proof?(ast) do
    {_, found} =
      Macro.prewalk(ast, false, fn
        {name, _, args} = ast, _
        when is_list(args) and name in [:ghost, :assert, :unfold, :assume, :havoc, :block] ->
          {ast, true}

        ast, found ->
          {ast, found}
      end)

    found
  end

  defp module_name({:__aliases__, _, [:"Elixir" | parts]}),
    do: module_id(Enum.join([:"Elixir" | parts], "."))

  defp module_name({:__aliases__, _, parts}),
    do: module_id("Elixir." <> Enum.join(parts, "."))

  defp module_name(name) when is_atom(name), do: module_id(Atom.to_string(name))
  defp module_id("Elixir." <> name), do: name
  defp module_id(name), do: ":" <> name

  defp expr(value, _path) when is_integer(value),
    do: %{kind: "int", value: Integer.to_string(value), line: 0}

  defp expr(value, _path) when is_atom(value),
    do: %{kind: "atom", value: Atom.to_string(value), line: 0}

  defp expr({:__block__, meta, items}, path),
    do: node("block", meta, items: Enum.map(items, &expr(&1, path)))

  defp expr({:if, meta, [cond, opts]}, path) do
    node("branch", meta,
      cond: expr(cond, path),
      yes: expr(Keyword.fetch!(opts, :do), path),
      no: expr(opts[:else], path)
    )
  end

  defp expr({:case, meta, [value, [do: arms]]}, path) do
    node("case", meta, value: expr(value, path), arms: Enum.map(arms, &arm(&1, path)))
  end

  defp expr({:=, meta, [pattern, value]}, path),
    do: node("bind", meta, pattern: expr(pattern, path), value: expr(value, path))

  defp expr({kind, meta, [vars, [do: body]]}, path) when kind in [:forall, :exists] do
    vars = if is_list(vars), do: vars, else: [vars]

    names =
      Enum.map(vars, fn
        {name, _, ctx} when is_atom(name) and is_atom(ctx) and name != :_ -> Atom.to_string(name)
        ast -> fail(path, line(ast), "quantifiers bind named variables")
      end)

    if names == [] or length(names) != length(Enum.uniq(names)),
      do: fail(path, meta[:line], "quantifier variables must be distinct")

    node("quant", meta, all: kind == :forall, vars: names, body: expr(body, path))
  end

  defp expr({:assume, meta, [value]}, path), do: node("assume", meta, value: expr(value, path))

  defp expr({:havoc, meta, args}, path) when is_list(args) do
    names =
      Enum.map(args, fn
        {name, _, ctx} when is_atom(name) and is_atom(ctx) and name != :_ -> Atom.to_string(name)
        ast -> fail(path, line(ast), "havoc requires named variables")
      end)

    node("havoc", meta, names: names)
  end

  defp expr({:block, meta, [[do: body]]}, path), do: node("local", meta, body: expr(body, path))

  defp expr({:ghost, meta, [[do: body]]}, path), do: node("ghost", meta, body: expr(body, path))
  defp expr({:assert, meta, [value]}, path), do: node("assert", meta, value: expr(value, path))

  defp expr({:assert, meta, [value, message]}, path) when is_binary(message),
    do: node("assert", meta, value: expr(value, path), message: message)

  defp expr({:unfold, meta, [call]}, path), do: node("unfold", meta, call: expr(call, path))

  defp expr({:{}, meta, items}, path),
    do: node("tuple", meta, items: Enum.map(items, &expr(&1, path)))

  defp expr({a, b}, path), do: node("tuple", [], items: Enum.map([a, b], &expr(&1, path)))

  defp expr(value, path) when is_list(value) do
    {items, tail} = list(value)

    node("list", [],
      items: Enum.map(items, &expr(&1, path)),
      tail:
        case tail do
          :none -> nil
          {:some, value} -> expr(value, path)
        end
    )
  end

  defp expr({name, meta, ctx}, _path) when is_atom(name) and is_atom(ctx),
    do: node("var", meta, name: Atom.to_string(name))

  defp expr({name, meta, args}, path) when (name in @ops or name == :term_size) and is_list(args),
    do: node("op", meta, name: Atom.to_string(name), args: Enum.map(args, &expr(&1, path)))

  defp expr({{:., _, [module, name]}, meta, args}, path) when is_atom(name) and is_list(args) do
    mod = module_name(module)

    if (mod == "Kernel" and name in @ops) or (mod == ":erlang" and name in @erl) do
      node("op", meta, name: Atom.to_string(name), args: Enum.map(args, &expr(&1, path)))
    else
      node("call", meta,
        module: mod,
        name: Atom.to_string(name),
        args: Enum.map(args, &expr(&1, path))
      )
    end
  end

  defp expr({name, meta, args}, path) when is_atom(name) and is_list(args),
    do:
      node("call", meta,
        module: nil,
        name: Atom.to_string(name),
        args: Enum.map(args, &expr(&1, path))
      )

  defp expr(ast, path),
    do: fail(path, line(ast), "unsupported expression: #{Macro.to_string(ast)}")

  defp arm({:->, _, [[{:when, _, [pattern, guard]}], body]}, path),
    do: %{pattern: expr(pattern, path), guard: expr(guard, path), body: expr(body, path)}

  defp arm({:->, _, [[pattern], body]}, path),
    do: %{pattern: expr(pattern, path), guard: expr(true, path), body: expr(body, path)}

  defp arm(ast, path), do: fail(path, line(ast), "unsupported case clause")

  defp list([]), do: {[], :none}
  defp list([{:|, _, [head, tail]}]), do: {[head], {:some, tail}}

  defp list([head | rest]) do
    {items, tail} = list(rest)
    {[head | items], tail}
  end

  defp block({:__block__, _, items}), do: items
  defp block(item), do: [item]
  defp node(kind, meta, attrs), do: Map.new([kind: kind, line: meta[:line] || 0] ++ attrs)
  defp line({_, meta, _}) when is_list(meta), do: meta[:line] || 1
  defp line(_), do: 1

  defp fail(path, line, message),
    do: raise(CompileError, file: path, line: line || 1, description: message)
end
