---------------- MODULE Lifecycle ----------------
EXTENDS Naturals
VARIABLES phase, game, old, new
vars == <<phase, game, old, new>>
Init == /\ phase = "empty" /\ game = "none" /\ old = "none" /\ new = "none"
PrepareInstall == /\ phase = "empty" /\ phase' = "install-ready" /\ new' = "new" /\ UNCHANGED <<game,old>>
Install == /\ phase = "install-ready" /\ phase' = "install-placed" /\ game' = "new" /\ new' = "none" /\ UNCHANGED old
FinishInstall == /\ phase = "install-placed" /\ phase' = "installed" /\ UNCHANGED <<game,old,new>>
RecoverInstall == /\ phase = "install-placed" /\ phase' = "install-ready" /\ new' = game /\ game' = "none" /\ UNCHANGED old
CancelInstall == /\ phase = "install-ready" /\ phase' = "empty" /\ new' = "none" /\ UNCHANGED <<game,old>>
PrepareRemove == /\ phase = "installed" /\ phase' = "remove-ready" /\ UNCHANGED <<game,old,new>>
Remove == /\ phase = "remove-ready" /\ phase' = "remove-moved" /\ old' = game /\ game' = "none" /\ UNCHANGED new
FinishRemove == /\ phase = "remove-moved" /\ phase' = "removed" /\ UNCHANGED <<game,old,new>>
RestoreRemove == /\ phase = "remove-moved" /\ phase' = "installed" /\ game' = old /\ old' = "none" /\ UNCHANGED new
RecoverRemove == /\ phase = "remove-ready" /\ phase' = "installed" /\ UNCHANGED <<game,old,new>>
PrepareUpdate == /\ phase = "installed" /\ phase' = "update-ready" /\ new' = "new" /\ UNCHANGED <<game,old>>
SaveOld == /\ phase = "update-ready" /\ phase' = "old-saved" /\ old' = game /\ game' = "none" /\ UNCHANGED new
PlaceNew == /\ phase = "old-saved" /\ phase' = "new-placed" /\ game' = new /\ new' = "none" /\ UNCHANGED old
FinishUpdate == /\ phase = "new-placed" /\ phase' = "updated" /\ UNCHANGED <<game,old,new>>
RecoverBeforeSave == /\ phase = "update-ready" /\ phase' = "installed" /\ new' = "none" /\ UNCHANGED <<game,old>>
RecoverAfterSave == /\ phase = "old-saved" /\ phase' = "installed" /\ game' = old /\ old' = "none" /\ new' = "none"
RecoverAfterPlace == /\ phase = "new-placed" /\ phase' = "old-saved" /\ new' = game /\ game' = "none" /\ UNCHANGED old
Done == /\ phase \in {"updated", "removed"} /\ UNCHANGED vars
Next == PrepareInstall \/ Install \/ FinishInstall \/ RecoverInstall \/ CancelInstall \/ PrepareRemove \/ Remove \/ FinishRemove \/ RestoreRemove \/ RecoverRemove \/ PrepareUpdate \/ SaveOld \/ PlaceNew \/ FinishUpdate \/ RecoverBeforeSave \/ RecoverAfterSave \/ RecoverAfterPlace \/ Done
OldPreserved == phase \in {"update-ready", "old-saved", "new-placed", "updated"} => game # "none" \/ old # "none"
BackupOnUpdate == phase \in {"old-saved", "new-placed", "updated"} => old # "none"
InstalledHasFiles == phase \in {"installed", "updated"} => game # "none"
RemovedKept == phase = "removed" => /\ game = "none" /\ old # "none"
=============================================================================
