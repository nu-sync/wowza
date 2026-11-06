---------------------------- MODULE Plan ----------------------------
EXTENDS Naturals
VARIABLES phase, selectedRoot, planRoot, revision, expectedRevision,
          authorized, writtenRoot, writes
vars == <<phase, selectedRoot, planRoot, revision, expectedRevision,
          authorized, writtenRoot, writes>>
Init == /\ phase = "ready"
        /\ selectedRoot = "A" /\ planRoot = "none"
        /\ revision = 0 /\ expectedRevision = 0
        /\ authorized = FALSE /\ writtenRoot = "none" /\ writes = FALSE
Preview == /\ phase = "ready" /\ phase' = "preview"
           /\ planRoot' = selectedRoot /\ expectedRevision' = revision
           /\ UNCHANGED <<selectedRoot, revision, authorized, writtenRoot, writes>>
ChangeInputs == /\ phase \in {"preview", "confirmed"} /\ revision = 0
                /\ revision' = 1
                /\ UNCHANGED <<phase, selectedRoot, planRoot, expectedRevision, authorized, writtenRoot, writes>>
ChangeSelection == /\ phase \in {"preview", "confirmed"} /\ selectedRoot = "A"
                   /\ selectedRoot' = "B"
                   /\ UNCHANGED <<phase, planRoot, revision, expectedRevision, authorized, writtenRoot, writes>>
Confirm == /\ phase = "preview" /\ phase' = "confirmed" /\ authorized' = TRUE
           /\ UNCHANGED <<selectedRoot, planRoot, revision, expectedRevision, writtenRoot, writes>>
Cancel == /\ phase = "preview" /\ phase' = "cancelled"
          /\ UNCHANGED <<selectedRoot, planRoot, revision, expectedRevision, authorized, writtenRoot, writes>>
DryRun == /\ phase = "preview" /\ phase' = "preview_only"
          /\ UNCHANGED <<selectedRoot, planRoot, revision, expectedRevision, authorized, writtenRoot, writes>>
Apply == /\ phase = "confirmed" /\ revision = expectedRevision
         /\ phase' = "applied" /\ writes' = TRUE /\ writtenRoot' = planRoot
         /\ UNCHANGED <<selectedRoot, planRoot, revision, expectedRevision, authorized>>
RejectStale == /\ phase = "confirmed" /\ revision # expectedRevision
               /\ phase' = "rejected"
               /\ UNCHANGED <<selectedRoot, planRoot, revision, expectedRevision, authorized, writtenRoot, writes>>
Done == /\ phase \in {"cancelled", "preview_only", "applied", "rejected"} /\ UNCHANGED vars
Next == Preview \/ ChangeInputs \/ ChangeSelection \/ Confirm \/ Cancel \/ DryRun \/ Apply \/ RejectStale \/ Done
ConfirmationRequired == writes => authorized
SameInstallation == writes => writtenRoot = planRoot
FreshInputsRequired == writes => revision = expectedRevision
ReadOnlyPreview == phase \in {"ready", "preview", "cancelled", "preview_only", "rejected"} => ~writes
=============================================================================
