---- MODULE counter ----
EXTENDS Integers, vex
CONSTANT limit
VARIABLE balance
vars == <<balance>>

init == balance = 0

\* Each request is atomic. The pure decision comes from counter.ex.
request(delta) == /\ step_pre(balance, delta)
                  /\ balance' = step(balance, delta).ival
                  /\ balance' <= limit

next == \E delta \in {-1, 1}: request(delta)
spec == init /\ [][next]_vars
safe == balance \in 0..limit
====
