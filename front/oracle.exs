# Runtime reference for conformance tests. Only the listed built-ins may execute.
rows = System.argv() |> hd() |> File.read!() |> JSON.decode!()

results = Enum.map(rows, fn %{"op" => op, "args" => args} ->
  args = Enum.map(args, &String.to_integer/1)
  value = case {op, args} do
    {"div", [a, b]} -> div(a, b)
    {"rem", [a, b]} -> rem(a, b)
    {"+", [a, b]} -> a + b
    {"-", [a, b]} -> a - b
    {"*", [a, b]} -> a * b
    _ -> raise "unsupported oracle operation"
  end
  Integer.to_string(value)
end)

IO.puts(JSON.encode!(results))
