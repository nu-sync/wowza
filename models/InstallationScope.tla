----------------------- MODULE InstallationScope -----------------------
CONSTANT ValidateScope
VARIABLES phase, redirected, writes, destination
vars == <<phase, redirected, writes, destination>>
Init == /\ phase = "ready" /\ redirected = FALSE /\ writes = FALSE /\ destination = "none"
Redirect == /\ phase \in {"ready", "preview"} /\ ~redirected /\ redirected' = TRUE
            /\ UNCHANGED <<phase, writes, destination>>
Preview == /\ phase = "ready"
           /\ phase' = IF ValidateScope /\ redirected THEN "refused" ELSE "preview"
           /\ UNCHANGED <<redirected, writes, destination>>
Confirm == /\ phase = "preview" /\ phase' = "confirmed"
           /\ UNCHANGED <<redirected, writes, destination>>
Apply == /\ phase = "confirmed"
         /\ IF ValidateScope /\ redirected
               THEN /\ phase' = "refused" /\ UNCHANGED <<writes, destination>>
               ELSE /\ phase' = "applied" /\ writes' = TRUE
                    /\ destination' = IF redirected THEN "other" ELSE "selected"
         /\ UNCHANGED redirected
Cancel == /\ phase = "preview" /\ phase' = "cancelled"
          /\ UNCHANGED <<redirected, writes, destination>>
Done == /\ phase \in {"refused", "applied", "cancelled"} /\ UNCHANGED vars
Next == Redirect \/ Preview \/ Confirm \/ Apply \/ Cancel \/ Done
SameInstallation == writes => destination = "selected"
NoWritesWhenRefused == phase \in {"refused", "cancelled"} => ~writes
=============================================================================
