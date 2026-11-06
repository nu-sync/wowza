---------------- MODULE Handoff ----------------
VARIABLES state, confirmed, opened
vars == <<state, confirmed, opened>>
Init == /\ state = "input" /\ confirmed = FALSE /\ opened = FALSE
Validate == /\ state = "input" /\ state' = "preview" /\ UNCHANGED <<confirmed,opened>>
RejectInvalid == /\ state = "input" /\ state' = "invalid" /\ UNCHANGED <<confirmed,opened>>
Confirm == /\ state = "preview" /\ state' = "opening" /\ confirmed' = TRUE /\ UNCHANGED opened
Cancel == /\ state = "preview" /\ state' = "cancelled" /\ UNCHANGED <<confirmed,opened>>
Open == /\ state = "opening" /\ state' = "opened" /\ opened' = TRUE /\ UNCHANGED confirmed
OpenError == /\ state = "opening" /\ state' = "failed" /\ UNCHANGED <<confirmed,opened>>
Done == /\ state \in {"invalid","cancelled","opened","failed"} /\ UNCHANGED vars
Next == Validate \/ RejectInvalid \/ Confirm \/ Cancel \/ Open \/ OpenError \/ Done
OpenRequiresConsent == opened => confirmed
ReportTruth == (state = "opened") <=> opened
NoOpenIfCancelled == state = "cancelled" => ~opened
=============================================================================
