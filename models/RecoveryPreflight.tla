----------------------- MODULE RecoveryPreflight -----------------------
CONSTANT ValidatePreview
VARIABLES phase, changed, writes, acceptedInvalid
vars == <<phase, changed, writes, acceptedInvalid>>
Init == /\ phase = "ready" /\ changed = FALSE /\ writes = FALSE /\ acceptedInvalid = FALSE
ChangeFiles == /\ phase \in {"ready", "preview"} /\ ~changed /\ changed' = TRUE
               /\ UNCHANGED <<phase, writes, acceptedInvalid>>
Preview == /\ phase = "ready"
           /\ IF ValidatePreview /\ changed
                 THEN /\ phase' = "refused" /\ UNCHANGED acceptedInvalid
                 ELSE /\ phase' = "preview" /\ acceptedInvalid' = changed
           /\ UNCHANGED <<changed, writes>>
Confirm == /\ phase = "preview"
           /\ IF changed THEN /\ phase' = "refused" /\ UNCHANGED writes
                         ELSE /\ phase' = "restored" /\ writes' = TRUE
           /\ UNCHANGED <<changed, acceptedInvalid>>
Cancel == /\ phase = "preview" /\ phase' = "cancelled"
          /\ UNCHANGED <<changed, writes, acceptedInvalid>>
Done == /\ phase \in {"refused", "restored", "cancelled"} /\ UNCHANGED vars
Next == ChangeFiles \/ Preview \/ Confirm \/ Cancel \/ Done
HonestPreview == ~acceptedInvalid
NoChangedRecovery == writes => ~changed
NoWritesWhenRefused == phase \in {"refused", "cancelled"} => ~writes
=============================================================================
