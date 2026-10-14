# Branch accounting — X-1 (original explorer)

- test: `ledger::protocol::model_tests::x1_two_facts_two_writers_one_admission`
- source: this mission's raw output /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-mutation-tranche1-exec-01/receipts/03-tool-controls/run-e1-x1-pass-neg-selection-lists-two-tests/raw-output.txt, sha256 b507ddeef15b78b600115b5eeb97c290139d560adbf2578fe4b3a81d0e6a17ad
- header: `EXPLORATION X-1`
- vocabulary rows: 193; TAKEN 21, NOT TAKEN 108, NOT APPLICABLE 64
- raw: 21 listed taken, 69 listed NOT TAKEN, 0 printed taken outside the listed set
- TAKEN labels missing from the static BRANCHES constant: none
- verdict: **NORMALIZED**

## NOT APPLICABLE operations (absent by configuration)

- `DROP`: DROP false: step_fw offers the DROP option only when config.drop and the held ticket is Counted
- `append_gating`: gating callers = 0: no Gc actor (actors(); initial gc vector empty)
- `snapshot`: acknowledgement actors = 0: no Ack actor (actors())
- `append_acknowledgement`: acknowledgement actors = 0: no Ack actor (actors())
- `malformed snapshot`: acknowledgement actors = 0: no Ack actor (actors())

## Operation totals

| operation | rows | TAKEN | NOT TAKEN | NOT APPLICABLE |
|---|---|---|---|---|
| append_facts | 36 | 10 | 26 | 0 |
| expire | 1 | 1 | 0 | 0 |
| append_gating | 34 | 0 | 0 | 34 |
| lookup | 12 | 6 | 6 | 0 |
| explain | 11 | 0 | 11 | 0 |
| snapshot | 10 | 0 | 0 | 10 |
| append_acknowledgement | 16 | 0 | 0 | 16 |
| barrier | 5 | 2 | 3 | 0 |
| resolve | 15 | 0 | 15 | 0 |
| append_cell C | 14 | 0 | 14 | 0 |
| append_cell X | 14 | 0 | 14 | 0 |
| append_cell U | 14 | 0 | 14 | 0 |
| empty | 5 | 0 | 5 | 0 |
| admission | 2 | 2 | 0 | 0 |
| DROP | 1 | 0 | 0 | 1 |
| malformed snapshot | 3 | 0 | 0 | 3 |

## Rows

| id | operation | label | status | raw evidence | static |
|---|---|---|---|---|---|
| B001 | append_facts | `append_facts: Committed (inserted)` | TAKEN | listed taken | yes |
| B002 | append_facts | `append_facts: Committed (already present)` | TAKEN | listed taken | yes |
| B003 | append_facts | `append_facts: NotApplied NotWritten Entry(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B004 | append_facts | `append_facts: NotApplied NotWritten Entry(Latched)` | TAKEN | listed taken | yes |
| B005 | append_facts | `append_facts: NotApplied NotWritten Entry(NotHealthy)` | TAKEN | listed taken | yes |
| B006 | append_facts | `append_facts: NotApplied NotWritten Entry(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B007 | append_facts | `append_facts: NotApplied NotWritten Entry(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B008 | append_facts | `append_facts: NotApplied NotWritten Entry(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B009 | append_facts | `append_facts: NotApplied NotWritten Conflict(new or retained cause IdentityContent)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B010 | append_facts | `append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)` | NOT TAKEN | listed NOT TAKEN | yes |
| B011 | append_facts | `append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)` | NOT TAKEN | listed NOT TAKEN | yes |
| B012 | append_facts | `append_facts: NotApplied NotWritten Conflict(explained)` | NOT TAKEN | listed NOT TAKEN | yes |
| B013 | append_facts | `append_facts: NotApplied NotWritten SessionClosed` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B014 | append_facts | `append_facts: NotApplied NotWritten Corrected` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B015 | append_facts | `append_facts: NotApplied NotWritten TableFull` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B016 | append_facts | `append_facts: NotApplied NotWritten Constraint` | TAKEN | listed taken | yes |
| B017 | append_facts | `append_facts: NotApplied NotWritten Storage` | TAKEN | listed taken | yes |
| B018 | append_facts | `append_facts: NotApplied Uncertain Entry(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B019 | append_facts | `append_facts: NotApplied Uncertain Entry(Latched)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B020 | append_facts | `append_facts: NotApplied Uncertain Entry(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B021 | append_facts | `append_facts: NotApplied Uncertain Entry(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B022 | append_facts | `append_facts: NotApplied Uncertain Entry(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B023 | append_facts | `append_facts: NotApplied Uncertain Entry(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B024 | append_facts | `append_facts: NotApplied Uncertain Conflict(new or retained cause IdentityContent)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B025 | append_facts | `append_facts: NotApplied Uncertain Conflict(new or retained cause DuplicateRole)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B026 | append_facts | `append_facts: NotApplied Uncertain Conflict(new or retained cause MissingPrepared)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B027 | append_facts | `append_facts: NotApplied Uncertain Conflict(explained)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B028 | append_facts | `append_facts: NotApplied Uncertain SessionClosed` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B029 | append_facts | `append_facts: NotApplied Uncertain Corrected` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B030 | append_facts | `append_facts: NotApplied Uncertain TableFull` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B031 | append_facts | `append_facts: NotApplied Uncertain Constraint` | TAKEN | listed taken | yes |
| B032 | append_facts | `append_facts: NotApplied Uncertain Storage` | TAKEN | listed taken | yes |
| B033 | append_facts | `append_facts: Rejected(OtherLedger)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B034 | append_facts | `append_facts: Rejected(NotArmed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B035 | append_facts | `append_facts: unwound after an insert was issued` | TAKEN | listed taken | yes |
| B036 | append_facts | `append_facts: unwound before any insert` | TAKEN | listed taken | yes |
| B037 | expire | `expire: Armed -> Counted` | TAKEN | listed taken | yes |
| B038 | append_gating | `append_gating: Committed (inserted)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B039 | append_gating | `append_gating: Committed (already present)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B040 | append_gating | `append_gating: Refused NotWritten Entry(NotOpen)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B041 | append_gating | `append_gating: Refused NotWritten Entry(Latched)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B042 | append_gating | `append_gating: Refused NotWritten Entry(NotHealthy)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B043 | append_gating | `append_gating: Refused NotWritten Entry(CellsBusy)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B044 | append_gating | `append_gating: Refused NotWritten Entry(Malformed)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B045 | append_gating | `append_gating: Refused NotWritten Entry(Expired)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B046 | append_gating | `append_gating: Refused NotWritten Conflict(new or retained cause IdentityContent)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B047 | append_gating | `append_gating: Refused NotWritten Conflict(new or retained cause DuplicateRole)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B048 | append_gating | `append_gating: Refused NotWritten Conflict(new or retained cause MissingPrepared)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B049 | append_gating | `append_gating: Refused NotWritten Conflict(explained)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B050 | append_gating | `append_gating: Refused NotWritten SessionClosed` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B051 | append_gating | `append_gating: Refused NotWritten Corrected` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B052 | append_gating | `append_gating: Refused NotWritten TableFull` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B053 | append_gating | `append_gating: Refused NotWritten Constraint` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B054 | append_gating | `append_gating: Refused NotWritten Storage` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B055 | append_gating | `append_gating: Refused Uncertain Entry(NotOpen)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B056 | append_gating | `append_gating: Refused Uncertain Entry(Latched)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B057 | append_gating | `append_gating: Refused Uncertain Entry(NotHealthy)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B058 | append_gating | `append_gating: Refused Uncertain Entry(CellsBusy)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B059 | append_gating | `append_gating: Refused Uncertain Entry(Malformed)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B060 | append_gating | `append_gating: Refused Uncertain Entry(Expired)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B061 | append_gating | `append_gating: Refused Uncertain Conflict(new or retained cause IdentityContent)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B062 | append_gating | `append_gating: Refused Uncertain Conflict(new or retained cause DuplicateRole)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B063 | append_gating | `append_gating: Refused Uncertain Conflict(new or retained cause MissingPrepared)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B064 | append_gating | `append_gating: Refused Uncertain Conflict(explained)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B065 | append_gating | `append_gating: Refused Uncertain SessionClosed` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B066 | append_gating | `append_gating: Refused Uncertain Corrected` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B067 | append_gating | `append_gating: Refused Uncertain TableFull` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B068 | append_gating | `append_gating: Refused Uncertain Constraint` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B069 | append_gating | `append_gating: Refused Uncertain Storage` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B070 | append_gating | `append_gating: unwound after an insert (U kept, Unwound)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B071 | append_gating | `append_gating: unwound before any insert (U emptied)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B072 | lookup | `lookup: Conflict(cause)` | NOT TAKEN | listed NOT TAKEN | yes |
| B073 | lookup | `lookup: Conflict(explained)` | NOT TAKEN | listed NOT TAKEN | yes |
| B074 | lookup | `lookup: Present` | TAKEN | listed taken | yes |
| B075 | lookup | `lookup: Absent` | TAKEN | listed taken | yes |
| B076 | lookup | `lookup: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B077 | lookup | `lookup: Refused(Latched)` | TAKEN | listed taken | yes |
| B078 | lookup | `lookup: Refused(NotHealthy)` | TAKEN | listed taken | yes |
| B079 | lookup | `lookup: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B080 | lookup | `lookup: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B081 | lookup | `lookup: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B082 | lookup | `lookup: Failed` | TAKEN | listed taken | yes |
| B083 | lookup | `lookup: unwound` | TAKEN | listed taken | yes |
| B084 | explain | `explain: Explained` | NOT TAKEN | listed NOT TAKEN | yes |
| B085 | explain | `explain: Pending` | NOT TAKEN | listed NOT TAKEN | yes |
| B086 | explain | `explain: Busy` | NOT TAKEN | listed NOT TAKEN | yes |
| B087 | explain | `explain: Unknown` | NOT TAKEN | listed NOT TAKEN | yes |
| B088 | explain | `explain: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B089 | explain | `explain: Refused(Latched)` | NOT TAKEN | listed NOT TAKEN | yes |
| B090 | explain | `explain: Refused(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B091 | explain | `explain: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B092 | explain | `explain: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B093 | explain | `explain: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B094 | explain | `explain: unwound` | NOT TAKEN | listed NOT TAKEN | yes |
| B095 | snapshot | `snapshot: Taken` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B096 | snapshot | `snapshot: Nothing` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B097 | snapshot | `snapshot: NotNow(NotOpen)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B098 | snapshot | `snapshot: NotNow(Latched)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B099 | snapshot | `snapshot: NotNow(NotHealthy)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B100 | snapshot | `snapshot: NotNow(CellsBusy)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B101 | snapshot | `snapshot: NotNow(Malformed)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B102 | snapshot | `snapshot: NotNow(Expired)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B103 | snapshot | `snapshot: Failed` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B104 | snapshot | `snapshot: unwound` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B105 | append_acknowledgement | `append_acknowledgement: Committed` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B106 | append_acknowledgement | `append_acknowledgement: Refused(OtherStore)` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B107 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(NotOpen))` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B108 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(Latched))` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B109 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(NotHealthy))` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B110 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(CellsBusy))` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B111 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(Malformed))` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B112 | append_acknowledgement | `append_acknowledgement: Refused(NotNow(Expired))` | NOT APPLICABLE | not printed (not a BRANCHES label, not taken) | NO |
| B113 | append_acknowledgement | `append_acknowledgement: Refused(Stale)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B114 | append_acknowledgement | `append_acknowledgement: Refused(Empty)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B115 | append_acknowledgement | `append_acknowledgement: Refused(Future)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B116 | append_acknowledgement | `append_acknowledgement: Refused(Set)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B117 | append_acknowledgement | `append_acknowledgement: NotApplied(NotWritten)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B118 | append_acknowledgement | `append_acknowledgement: NotApplied(Uncertain)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B119 | append_acknowledgement | `append_acknowledgement: unwound after an insert (U kept)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B120 | append_acknowledgement | `append_acknowledgement: unwound before any insert (U emptied)` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B121 | barrier | `barrier: unwound` | NOT TAKEN | listed NOT TAKEN | yes |
| B122 | barrier | `barrier: Passed` | TAKEN | listed taken | yes |
| B123 | barrier | `barrier: Failed` | TAKEN | listed taken | yes |
| B124 | barrier | `barrier: NotNeeded` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B125 | barrier | `barrier: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B126 | resolve | `resolve: Corrected` | NOT TAKEN | listed NOT TAKEN | yes |
| B127 | resolve | `resolve: ConflictPlaced` | NOT TAKEN | listed NOT TAKEN | yes |
| B128 | resolve | `resolve: Acknowledged` | NOT TAKEN | listed NOT TAKEN | yes |
| B129 | resolve | `resolve: Absent` | NOT TAKEN | listed NOT TAKEN | yes |
| B130 | resolve | `resolve: Retained` | NOT TAKEN | listed NOT TAKEN | yes |
| B131 | resolve | `resolve: Inconsistent` | NOT TAKEN | listed NOT TAKEN | yes |
| B132 | resolve | `resolve: Stale` | NOT TAKEN | listed NOT TAKEN | yes |
| B133 | resolve | `resolve: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B134 | resolve | `resolve: Refused(Latched)` | NOT TAKEN | listed NOT TAKEN | yes |
| B135 | resolve | `resolve: Refused(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B136 | resolve | `resolve: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B137 | resolve | `resolve: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B138 | resolve | `resolve: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B139 | resolve | `resolve: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B140 | resolve | `resolve: unwound` | NOT TAKEN | listed NOT TAKEN | yes |
| B141 | append_cell C | `append_cell C: Committed (inserted)` | NOT TAKEN | listed NOT TAKEN | yes |
| B142 | append_cell C | `append_cell C: Committed (already marked)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B143 | append_cell C | `append_cell C: Committed (present)` | NOT TAKEN | listed NOT TAKEN | yes |
| B144 | append_cell C | `append_cell C: NotWritten` | NOT TAKEN | listed NOT TAKEN | yes |
| B145 | append_cell C | `append_cell C: Uncertain` | NOT TAKEN | listed NOT TAKEN | yes |
| B146 | append_cell C | `append_cell C: Stale` | NOT TAKEN | listed NOT TAKEN | yes |
| B147 | append_cell C | `append_cell C: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B148 | append_cell C | `append_cell C: Refused(Latched)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B149 | append_cell C | `append_cell C: Refused(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B150 | append_cell C | `append_cell C: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B151 | append_cell C | `append_cell C: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B152 | append_cell C | `append_cell C: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B153 | append_cell C | `append_cell C: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B154 | append_cell C | `append_cell C: unwound` | NOT TAKEN | listed NOT TAKEN | yes |
| B155 | append_cell X | `append_cell X: Committed (inserted)` | NOT TAKEN | listed NOT TAKEN | yes |
| B156 | append_cell X | `append_cell X: Committed (already marked)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B157 | append_cell X | `append_cell X: Committed (present)` | NOT TAKEN | listed NOT TAKEN | yes |
| B158 | append_cell X | `append_cell X: NotWritten` | NOT TAKEN | listed NOT TAKEN | yes |
| B159 | append_cell X | `append_cell X: Uncertain` | NOT TAKEN | listed NOT TAKEN | yes |
| B160 | append_cell X | `append_cell X: Stale` | NOT TAKEN | listed NOT TAKEN | yes |
| B161 | append_cell X | `append_cell X: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B162 | append_cell X | `append_cell X: Refused(Latched)` | NOT TAKEN | listed NOT TAKEN | yes |
| B163 | append_cell X | `append_cell X: Refused(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B164 | append_cell X | `append_cell X: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B165 | append_cell X | `append_cell X: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B166 | append_cell X | `append_cell X: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B167 | append_cell X | `append_cell X: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B168 | append_cell X | `append_cell X: unwound` | NOT TAKEN | listed NOT TAKEN | yes |
| B169 | append_cell U | `append_cell U: Committed (inserted)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B170 | append_cell U | `append_cell U: Committed (already marked)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B171 | append_cell U | `append_cell U: Committed (present)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B172 | append_cell U | `append_cell U: NotWritten` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B173 | append_cell U | `append_cell U: Uncertain` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B174 | append_cell U | `append_cell U: Stale` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B175 | append_cell U | `append_cell U: Refused(NotOpen)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B176 | append_cell U | `append_cell U: Refused(Latched)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B177 | append_cell U | `append_cell U: Refused(NotHealthy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B178 | append_cell U | `append_cell U: Refused(CellsBusy)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B179 | append_cell U | `append_cell U: Refused(Malformed)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B180 | append_cell U | `append_cell U: Refused(Expired)` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B181 | append_cell U | `append_cell U: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B182 | append_cell U | `append_cell U: unwound` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B183 | empty | `empty: unwound` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B184 | empty | `empty: Done` | NOT TAKEN | listed NOT TAKEN | yes |
| B185 | empty | `empty: NotCommitted` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B186 | empty | `empty: Stale` | NOT TAKEN | listed NOT TAKEN | yes |
| B187 | empty | `empty: OtherLedger` | NOT TAKEN | not printed (not a BRANCHES label, not taken) | NO |
| B188 | admission | `admission: admitted` | TAKEN | listed taken | yes |
| B189 | admission | `admission: refused` | TAKEN | listed taken | yes |
| B190 | DROP | `DROP: a Counted ticket dropped` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B191 | malformed snapshot | `malformed snapshot: future` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B192 | malformed snapshot | `malformed snapshot: set` | NOT APPLICABLE | listed NOT TAKEN | yes |
| B193 | malformed snapshot | `malformed snapshot: empty` | NOT APPLICABLE | listed NOT TAKEN | yes |

## Raw branch lines (verbatim)

```
    [taken    ] append_facts: Committed (inserted)
    [taken    ] append_facts: Committed (already present)
    [taken    ] append_facts: NotApplied NotWritten Entry(NotHealthy)
    [taken    ] append_facts: NotApplied NotWritten Entry(Latched)
    [taken    ] append_facts: NotApplied NotWritten Storage
    [taken    ] append_facts: NotApplied NotWritten Constraint
    [taken    ] append_facts: NotApplied Uncertain Constraint
    [taken    ] append_facts: NotApplied Uncertain Storage
    [NOT TAKEN] append_facts: NotApplied NotWritten Conflict(new or retained cause DuplicateRole)
    [NOT TAKEN] append_facts: NotApplied NotWritten Conflict(new or retained cause MissingPrepared)
    [NOT TAKEN] append_facts: NotApplied NotWritten Conflict(explained)
    [taken    ] append_facts: unwound before any insert
    [taken    ] append_facts: unwound after an insert was issued
    [taken    ] expire: Armed -> Counted
    [NOT TAKEN] append_gating: Committed (inserted)
    [NOT TAKEN] append_gating: Refused NotWritten Entry(NotHealthy)
    [NOT TAKEN] append_gating: Refused NotWritten Entry(CellsBusy)
    [NOT TAKEN] append_gating: Refused NotWritten Entry(Latched)
    [NOT TAKEN] append_gating: Refused NotWritten Storage
    [NOT TAKEN] append_gating: Refused NotWritten Constraint
    [NOT TAKEN] append_gating: Refused Uncertain Storage
    [NOT TAKEN] append_gating: Refused Uncertain Constraint
    [NOT TAKEN] append_gating: Refused NotWritten Conflict(new or retained cause IdentityContent)
    [NOT TAKEN] append_gating: unwound after an insert (U kept, Unwound)
    [NOT TAKEN] append_gating: unwound before any insert (U emptied)
    [taken    ] lookup: Present
    [taken    ] lookup: Absent
    [NOT TAKEN] lookup: Conflict(cause)
    [NOT TAKEN] lookup: Conflict(explained)
    [taken    ] lookup: Refused(NotHealthy)
    [taken    ] lookup: Refused(Latched)
    [taken    ] lookup: Failed
    [taken    ] lookup: unwound
    [NOT TAKEN] explain: Explained
    [NOT TAKEN] explain: Pending
    [NOT TAKEN] explain: Busy
    [NOT TAKEN] explain: Unknown
    [NOT TAKEN] explain: Refused(Latched)
    [NOT TAKEN] explain: unwound
    [NOT TAKEN] snapshot: Taken
    [NOT TAKEN] snapshot: Nothing
    [NOT TAKEN] snapshot: NotNow(NotHealthy)
    [NOT TAKEN] snapshot: NotNow(Latched)
    [NOT TAKEN] snapshot: Failed
    [NOT TAKEN] snapshot: unwound
    [NOT TAKEN] append_acknowledgement: Committed
    [NOT TAKEN] append_acknowledgement: Refused(Stale)
    [NOT TAKEN] append_acknowledgement: Refused(Empty)
    [NOT TAKEN] append_acknowledgement: Refused(Future)
    [NOT TAKEN] append_acknowledgement: Refused(Set)
    [NOT TAKEN] append_acknowledgement: Refused(NotNow(NotHealthy))
    [NOT TAKEN] append_acknowledgement: Refused(NotNow(CellsBusy))
    [NOT TAKEN] append_acknowledgement: Refused(NotNow(Latched))
    [NOT TAKEN] append_acknowledgement: NotApplied(NotWritten)
    [NOT TAKEN] append_acknowledgement: NotApplied(Uncertain)
    [NOT TAKEN] append_acknowledgement: unwound after an insert (U kept)
    [NOT TAKEN] append_acknowledgement: unwound before any insert (U emptied)
    [taken    ] barrier: Passed
    [taken    ] barrier: Failed
    [NOT TAKEN] barrier: unwound
    [NOT TAKEN] resolve: Absent
    [NOT TAKEN] resolve: Corrected
    [NOT TAKEN] resolve: ConflictPlaced
    [NOT TAKEN] resolve: Acknowledged
    [NOT TAKEN] resolve: Retained
    [NOT TAKEN] resolve: Inconsistent
    [NOT TAKEN] resolve: Stale
    [NOT TAKEN] resolve: Refused(Latched)
    [NOT TAKEN] resolve: unwound
    [NOT TAKEN] append_cell C: Committed (inserted)
    [NOT TAKEN] append_cell C: Committed (present)
    [NOT TAKEN] append_cell C: NotWritten
    [NOT TAKEN] append_cell C: Uncertain
    [NOT TAKEN] append_cell C: Stale
    [NOT TAKEN] append_cell C: unwound
    [NOT TAKEN] append_cell X: Committed (inserted)
    [NOT TAKEN] append_cell X: Committed (present)
    [NOT TAKEN] append_cell X: NotWritten
    [NOT TAKEN] append_cell X: Uncertain
    [NOT TAKEN] append_cell X: Stale
    [NOT TAKEN] append_cell X: Refused(Latched)
    [NOT TAKEN] append_cell X: unwound
    [NOT TAKEN] empty: Done
    [NOT TAKEN] empty: Stale
    [taken    ] admission: admitted
    [taken    ] admission: refused
    [NOT TAKEN] DROP: a Counted ticket dropped
    [NOT TAKEN] malformed snapshot: future
    [NOT TAKEN] malformed snapshot: set
    [NOT TAKEN] malformed snapshot: empty
```
