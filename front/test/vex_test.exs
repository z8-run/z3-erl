defmodule :vex_test do
  use ExUnit.Case

  test "contracts do not run during compilation; ghost code is erased" do
    Code.compile_string("""
    defmodule :vex_runtime_fixture do
      use :vex
      @verifier requires is_integer(n)
      @verifier ensures inc(n) === n + 1
      defv inc(n) do
        ghost do
          raise "must not execute"
        end
        unfold mathematical(n)
        n + 1
      end
      defvg mathematical(n), do: n + 1
    end
    """)

    assert apply(:vex_runtime_fixture, :inc, [8]) == 9
    refute function_exported?(:vex_runtime_fixture, :mathematical, 1)
  end

  test "parser never executes source and preserves large integer literals" do
    path = Path.join(System.tmp_dir!(), "vex-read-#{System.unique_integer([:positive])}.ex")
    on_exit(fn -> File.rm(path) end)

    File.write!(path, """
    raise "do not execute"
    defmodule Safe do
      use :vex
      @verifier ensures answer() === 123456789012345678901234567890
      defv answer(), do: 123456789012345678901234567890
      def unchecked(), do: :ok
    end
    """)

    result = :vex_read.files([path])
    assert [%{body: %{value: "123456789012345678901234567890"}}] = result.functions
    assert [%{owner: "Safe.unchecked/0"}] = result.skipped
  end
end
