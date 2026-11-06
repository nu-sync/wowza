---------------- MODULE Handoff ----------------
VARIABLES state, mode, validated, prompted, confirmed, opened
vars == <<state, mode, validated, prompted, confirmed, opened>>

Init == /\ state = "input"
        /\ mode \in {"browser", "url"}
        /\ validated = FALSE
        /\ prompted = FALSE
        /\ confirmed = FALSE
        /\ opened = FALSE

Validate == /\ state = "input"
            /\ state' = "ready"
            /\ validated' = TRUE
            /\ UNCHANGED <<mode, prompted, confirmed, opened>>
RejectInvalid == /\ state = "input"
                 /\ state' = "invalid"
                 /\ UNCHANGED <<mode, validated, prompted, confirmed, opened>>
Prompt == /\ mode = "browser"
          /\ state = "ready"
          /\ state' = "prompting"
          /\ prompted' = TRUE
          /\ UNCHANGED <<mode, validated, confirmed, opened>>
Confirm == /\ mode = "browser"
           /\ state \in {"ready", "prompting"}
           /\ state' = "opening"
           /\ confirmed' = TRUE
           /\ UNCHANGED <<mode, validated, prompted, opened>>
Cancel == /\ state = "prompting"
          /\ state' = "cancelled"
          /\ UNCHANGED <<mode, validated, prompted, confirmed, opened>>
Open == /\ state = "opening"
        /\ state' = "opened"
        /\ opened' = TRUE
        /\ UNCHANGED <<mode, validated, prompted, confirmed>>
OpenError == /\ state = "opening"
             /\ state' = "failed"
             /\ UNCHANGED <<mode, validated, prompted, confirmed, opened>>
ReturnURL == /\ mode = "url"
             /\ state = "ready"
             /\ state' = "returned"
             /\ UNCHANGED <<mode, validated, prompted, confirmed, opened>>
Done == /\ state \in {"invalid", "cancelled", "opened", "failed", "returned"}
        /\ UNCHANGED vars
Next == Validate \/ RejectInvalid \/ Prompt \/ Confirm \/ Cancel \/ Open \/ OpenError \/ ReturnURL \/ Done

OpenRequiresConsent == opened => confirmed
ReportTruth == (state = "opened") <=> opened
NoOpenIfCancelled == state = "cancelled" => ~opened
NoPromptInURLMode == mode = "url" => ~prompted
NoBrowserInURLMode == mode = "url" => (~confirmed /\ ~opened)
EffectsRequireValidation == (prompted \/ confirmed \/ opened \/ state = "returned") => validated
=============================================================================
