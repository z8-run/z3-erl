defmodule :vex_mix do
  use Mix.Project

  def project do
    [app: :vex, version: "0.1.0", elixir: ">= 1.18.0", deps: []]
  end

  def application, do: [extra_applications: [:crypto]]
end
