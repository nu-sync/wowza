------------------------ MODULE RecoveryChoice ------------------------
EXTENDS Naturals, FiniteSets
VARIABLES phase, pending, chosen, authorized, changed, writes, root, planRoot, writtenRoot
vars == <<phase, pending, chosen, authorized, changed, writes, root, planRoot, writtenRoot>>
Init == /\ phase = "inspect" /\ pending \in {{}, {"one"}, {"one", "two"}}
        /\ chosen = "none" /\ authorized = FALSE /\ changed = FALSE /\ writes = FALSE
        /\ root = "A" /\ planRoot = "none" /\ writtenRoot = "none"
NoPending == /\ phase = "inspect" /\ pending = {} /\ phase' = "none"
             /\ UNCHANGED <<pending, chosen, authorized, changed, writes, root, planRoot, writtenRoot>>
SelectOnly == /\ phase = "inspect" /\ Cardinality(pending) = 1
              /\ chosen' = (CHOOSE id \in pending: TRUE) /\ planRoot' = root /\ phase' = "preview"
              /\ UNCHANGED <<pending, authorized, changed, writes, root, writtenRoot>>
NeedChoice == /\ phase = "inspect" /\ Cardinality(pending) > 1 /\ phase' = "choose"
              /\ UNCHANGED <<pending, chosen, authorized, changed, writes, root, planRoot, writtenRoot>>
SelectNamed == /\ phase = "choose" /\ chosen' \in pending /\ planRoot' = root /\ phase' = "preview"
               /\ UNCHANGED <<pending, authorized, changed, writes, root, writtenRoot>>
Cancel == /\ phase = "preview" /\ phase' = "cancelled"
          /\ UNCHANGED <<pending, chosen, authorized, changed, writes, root, planRoot, writtenRoot>>
ChangeJournal == /\ phase = "preview" /\ ~changed /\ changed' = TRUE
                 /\ UNCHANGED <<phase, pending, chosen, authorized, writes, root, planRoot, writtenRoot>>
ChangeDefault == /\ phase = "preview" /\ root = "A" /\ root' = "B"
                 /\ UNCHANGED <<phase, pending, chosen, authorized, changed, writes, planRoot, writtenRoot>>
Recover == /\ phase = "preview" /\ ~changed /\ phase' = "recovered"
           /\ authorized' = TRUE /\ writes' = TRUE /\ writtenRoot' = planRoot
           /\ UNCHANGED <<pending, chosen, changed, root, planRoot>>
RefuseChanged == /\ phase = "preview" /\ changed /\ phase' = "refused" /\ authorized' = TRUE
                 /\ UNCHANGED <<pending, chosen, changed, writes, root, planRoot, writtenRoot>>
Done == /\ phase \in {"none", "cancelled", "recovered", "refused"} /\ UNCHANGED vars
Next == NoPending \/ SelectOnly \/ NeedChoice \/ SelectNamed \/ Cancel \/ ChangeJournal \/ ChangeDefault \/ Recover \/ RefuseChanged \/ Done
KnownOperation == writes => chosen \in pending
ConfirmationRequired == writes => authorized
UnchangedJournal == writes => ~changed
SameInstallation == writes => writtenRoot = planRoot
NoAmbiguousWrites == phase \in {"inspect", "choose", "none", "cancelled", "refused"} => ~writes
=============================================================================
