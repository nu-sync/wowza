---------------- MODULE Update ----------------
EXTENDS Naturals, FiniteSets
CONSTANT Folders
VARIABLES phase, game, backup, staged
vars == <<phase, game, backup, staged>>
Init == /\ phase = "ready" /\ game = [f \in Folders |-> "old"] /\ backup = [f \in Folders |-> "none"] /\ staged = [f \in Folders |-> "new"]
SaveOld(f) == /\ phase = "ready" /\ game[f] = "old" /\ backup[f] = "none" /\ game' = [game EXCEPT ![f] = "none"] /\ backup' = [backup EXCEPT ![f] = "old"] /\ UNCHANGED <<phase,staged>>
PlaceNew(f) == /\ phase = "ready" /\ \A g \in Folders: backup[g] = "old" /\ game[f] = "none" /\ staged[f] = "new" /\ game' = [game EXCEPT ![f] = "new"] /\ staged' = [staged EXCEPT ![f] = "none"] /\ UNCHANGED <<phase,backup>>
Commit == /\ phase = "ready" /\ \A f \in Folders: game[f] = "new" /\ phase' = "committed" /\ UNCHANGED <<game,backup,staged>>
BeginRecovery == /\ phase = "ready" /\ phase' = "recovering" /\ UNCHANGED <<game,backup,staged>>
ReturnNew(f) == /\ phase = "recovering" /\ game[f] = "new" /\ staged[f] = "none" /\ game' = [game EXCEPT ![f] = "none"] /\ staged' = [staged EXCEPT ![f] = "new"] /\ UNCHANGED <<phase,backup>>
RestoreOld(f) == /\ phase = "recovering" /\ \A g \in Folders: game[g] # "new" /\ backup[f] = "old" /\ game[f] = "none" /\ game' = [game EXCEPT ![f] = "old"] /\ backup' = [backup EXCEPT ![f] = "none"] /\ UNCHANGED <<phase,staged>>
EndRecovery == /\ phase = "recovering" /\ \A f \in Folders: game[f] = "old" /\ phase' = "restored" /\ UNCHANGED <<game,backup,staged>>
Done == /\ phase \in {"committed", "restored"} /\ UNCHANGED vars
Next == (\E f \in Folders: SaveOld(f)) \/ (\E f \in Folders: PlaceNew(f)) \/ Commit \/ BeginRecovery \/ (\E f \in Folders: ReturnNew(f)) \/ (\E f \in Folders: RestoreOld(f)) \/ EndRecovery \/ Done
OldPreserved == \A f \in Folders: game[f] = "old" \/ backup[f] = "old"
NewPreserved == \A f \in Folders: game[f] = "new" \/ staged[f] = "new"
NoMixedAfterCommit == phase = "committed" => \A f \in Folders: game[f] = "new" /\ backup[f] = "old"
RestoredIsOriginal == phase = "restored" => \A f \in Folders: game[f] = "old"
NoDuplicateOld == \A f \in Folders: ~(game[f] = "old" /\ backup[f] = "old")
=============================================================================
