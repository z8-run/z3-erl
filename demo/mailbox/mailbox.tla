---- MODULE mailbox ----
EXTENDS Naturals, Sequences, FiniteSets
CONSTANT actors, messages, bound
VARIABLE boxes, delivered
vars == <<boxes, delivered>>

init == /\ boxes = [p \in actors |-> <<>>]
        /\ delivered = {}

send(p, m) == /\ Len(boxes[p]) < bound
              /\ boxes' = [boxes EXCEPT ![p] = Append(@, m)]
              /\ UNCHANGED delivered

receive(p) == /\ Len(boxes[p]) > 0
               /\ delivered' = delivered \cup {Head(boxes[p])}
               /\ boxes' = [boxes EXCEPT ![p] = Tail(@)]

next == (\E p \in actors, m \in messages: send(p, m))
        \/ (\E p \in actors: receive(p))
base == init /\ [][next]_vars
spec == base /\ (\A p \in actors: WF_vars(receive(p)))
safe == /\ delivered \subseteq messages
        /\ \A p \in actors: Len(boxes[p]) <= bound
pending(m) == \E p \in actors: \E i \in 1..Len(boxes[p]): boxes[p][i] = m
live == \A m \in messages: pending(m) ~> (m \in delivered)
====
