---- MODULE race ----
EXTENDS Naturals
VARIABLE pc, seen, value
vars == <<pc, seen, value>>
actors == {1, 2}
init == /\ pc = [p \in actors |-> 0]
        /\ seen = [p \in actors |-> 0]
        /\ value = 0
read(p) == /\ pc[p] = 0
           /\ seen' = [seen EXCEPT ![p] = value]
           /\ pc' = [pc EXCEPT ![p] = 1]
           /\ UNCHANGED value
write(p) == /\ pc[p] = 1
            /\ value' = seen[p] + 1
            /\ pc' = [pc EXCEPT ![p] = 2]
            /\ UNCHANGED seen
next == (\E p \in actors: read(p) \/ write(p)) \/ UNCHANGED vars
spec == init /\ [][next]_vars
safe == (\A p \in actors: pc[p] = 2) => value = 2
====
