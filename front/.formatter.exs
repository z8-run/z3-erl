[
  inputs: ["mix.exs", "{lib,test}/**/*.{ex,exs}", "../demo/dafny/*.ex"],
  locals_without_parens: [requires: 1, ensures: 1, decreases: 1, assert: 1, unfold: 1]
]
