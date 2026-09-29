Code.require_file("lib/read.ex", __DIR__)

try do
  {opts, args, invalid} = OptionParser.parse(System.argv(), strict: [manifest: :string, out: :string])
  if invalid != [], do: raise(ArgumentError, "invalid frontend options")
  paths = if opts[:manifest], do: JSON.decode!(File.read!(opts[:manifest])), else: args
  json = paths |> :vex_read.files() |> JSON.encode!()
  if opts[:out], do: File.write!(opts[:out], json), else: IO.puts(json)
rescue
  error ->
    IO.puts(:stderr, Exception.message(error))
    System.halt(2)
end
