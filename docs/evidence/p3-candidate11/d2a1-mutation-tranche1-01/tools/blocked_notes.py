"""Executor observations for the 21 controls with no authenticated exact definition (mission 7).

These are NOT definitions. No anchor, replacement or patch is given for any of them, because no authorised source
(the contract, AD-1-R4, AD-1-R5, the sealed controls_defs.py) gives one, and the mission forbids inventing one.

For each control: where in Candidate C the contract's one-line change would land (`sites`: exact strings, used only
to locate lines at C, never as anchors of a mutation), and the questions an exact definition must settle (`open`).
derive_catalog.py records each site's line numbers and occurrence count at C.
"""
P = "crates/nexus-governed-control/src/ledger/protocol.rs"
M = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"

NOTES = {
    "C3a": {
        "sites": [(P, "        let refusal = if set.frontier != now.frontier {")],
        "open": [
            "Publish::acknowledgement carries source markers only for tests 0 and 1 (`// Test 0.`, `// Test 1.`); "
            "nothing in the source binds contract 4.8 test 2 to a code clause. The chain after `recheck` has four "
            "clauses (Stale, Empty, Future, Set) in the contract's order, but the binding is an executor reading.",
            "Test 2 is the chain's head clause; removing it means rewriting `let refusal = if ...` so that the chain "
            "starts at test 3. No authorised source gives that text.",
        ],
    },
    "C3b": {
        "sites": [(P, "        } else if set.top > set.head || set.head > now.head {")],
        "open": [
            "As C3a, the binding of contract test 4 to this clause is not marked in the source.",
            "Contract test 4 is two comparisons (N at most the snapshot's head; that head at most the head now). "
            "Whether 'test 4 removed' removes the whole clause or one comparison is not stated in an authorised text.",
        ],
    },
    "C3c": {
        "sites": [(P, "        } else if now.count != set.count || now.digest != set.digest {")],
        "open": [
            "As C3a, the binding of contract test 5 to this clause is not marked in the source.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C5": {
        "sites": [(P, "    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {"),
                  (P, "            Read::Got(Some(_)) => match io.frontier() {")],
        "open": [
            "'Raises the frontier with neither the presence read nor the frontier check': whether the verbs "
            "`acknowledgement` and `frontier` are still issued (the explorer counts verbs, E2 and E5) or removed.",
            "What resolve then answers (Acknowledged with N, or another variant), and whether the Absent, Failed and "
            "Corrupt arms remain reachable.",
            "Several encodings with different observable verb sequences fit the one-line text.",
        ],
    },
    "C6": {
        "sites": [(P, "        if let Some(cause) = self.refusals.get(key) {"),
                  (P, "        if let Some(known) = self.known(&member.key, facts.explained) {")],
        "open": [
            "Classification row b is evaluated through `Core::known`, which also serves lookup (4.6) and is read by "
            "the model itself (`step_gc` calls `w.core.known`). Whether the change is confined to classification "
            "(the `classify` call site) or made in `known` for every caller is not stated.",
            "'Ignores the refusal table' leaves X's explanation and the ledger's explanation in row b; the exact text "
            "is not given by an authorised source.",
        ],
    },
    "C7": {
        "sites": [(P, "            Read::Got(Some(seq)) => {\n                self.core.refusals.remove(key);\n"
                      "                return Explain::Explained(seq);")],
        "open": [
            "'Explained removes every table entry': whether only explain's Explained path changes, or also the "
            "other removal in `settle` (X's own append committing).",
            "The form (clear the whole table, or remove every entry but keep the call) is not given.",
        ],
    },
    "C8": {
        "sites": [(P, "        // Step 3: every member classified before any insert."), (P, "        // Step 5.")],
        "open": [
            "'Members are classified and inserted one at a time' needs steps 3 to 5 of `Publish::facts` merged into "
            "one loop. Step 4's no-insert path, the rollback and persistence when a later member is refused after "
            "an earlier insert, and the `issued` / `conflict` bookkeeping are each unspecified.",
            "This is a restructuring, not a token change; any text would be executor-authored.",
        ],
    },
    "C11": {
        "sites": [(P, "                Insert::Constraint => {\n                    self.core.latch(LatchCause::Constraint);\n"
                      "                    Reason::Constraint\n                }"),
                  (P, "            Insert::Constraint => {\n                self.core.latch(LatchCause::Constraint);\n"
                      "                Reason::Constraint\n            }"),
                  (P, "                if failed == Insert::Constraint {")],
        "open": [
            "Scope: the fact append's step 5 only (the first site), or every append with an insert constraint path: "
            "the gating append runs steps 2 to 6 of 4.4 for one member (the second site, the same code at another "
            "indentation), and the acknowledgement append and the cell appends have their own `Insert::Constraint` "
            "tests (the third site). The contract's 'during the inserts' does not settle it.",
            "Whether the one rollback is still issued (E5 counts it) and only its answer is ignored.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C15": {
        "sites": [(P, "            && self.occupancy(cell) == Some(token.occupancy)")],
        "open": [
            "'Ignores the occupancy number': whether the cell must still be occupied (`self.occupancy(cell)."
            "is_some()`) or the clause is dropped entirely; the two differ for a token naming an empty cell.",
            "`Core::names` serves resolve, the cell appends and empty; whether every cell operation is changed "
            "(the contract says 'a cell operation').",
            "AD-1-R5 F fixes C15's configuration, explorer, ceiling and restored side, not its text.",
        ],
    },
    "C16": {
        "sites": [(P, "            TicketState::Counted(_) => {\n                exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);")],
        "open": [
            "'The drop of a Counted ticket lowers the count': which count (`unapplied`, `live`, or both), and "
            "whether the handover-fault bit is still set.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C17": {
        "sites": [(P, "                let persistence = if self.issued && self.conflict.is_none() {\n"
                      "                    Persistence::Uncertain")],
        "open": [
            "'The guard reports Not written after an insert was issued': whether only this persistence changes, and "
            "whether `self.keep = self.issued;` (which keeps a gating or acknowledgement occupant in U on unwind) "
            "is left unchanged.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C18a": {
        "sites": [(P, "            if !self.keep {\n                self.core.u = None;"),
                  (P, "    fn gating(&mut self, record: &GatingRecord, io: &mut impl Txn) -> GatingOutcome {")],
        "open": [
            "'A refused gating append leaves U occupied': every refusal after U is placed (open failure, read "
            "failure, classification refusal, insert failure with Not written), or only the refusal E7 names "
            "(a conflict with a new cause).",
            "Site: the gating operation's return paths (setting `keep`), or the guard's emptying of U.",
            "Which exit mark the kept occupant carries (the guard writes ReturnedUncertain when `keep` is set).",
        ],
    },
    "C18b": {
        "sites": [(P, "        if gating {\n            if self.x.is_none() {\n                self.explanation(member.key, cause);")],
        "open": [
            "'A gating conflict makes a table entry': a direct insert into the table, or falling through to row "
            "d's fact path (which also applies the table-full check and its latch).",
            "Whether X's explanation is still built.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C20": {
        "sites": [(P, "            Read::Got(Some(seq)) => {\n                self.rolled(io.rollback());\n"
                      "                self.settle(cell, seq);")],
        "open": [
            "'An explanation found already present is not marked committed': whether only X's mark is withheld "
            "(C's present path unchanged), whether `settle`'s table-entry removal still happens, and what the "
            "operation answers.",
            "No authorised source gives the resulting text.",
        ],
    },
    "C23": {
        "sites": [(P, "            | bit(self.x.is_some(), X_OCCUPIED)")],
        "open": [
            "A single evident site, but no authorised source gives the resulting text (removing the line leaves "
            "the expression ending at C's bit); the executor may not supply it.",
        ],
    },
    "C26": {
        "sites": [(P, "        if !io.run() {\n            return BarrierOutcome::Failed;\n        }")],
        "open": [
            "'A passed barrier clears the latch': where the clearing is inserted (before or after the phase "
            "assignment) and its exact text. No authorised source gives it.",
        ],
    },
    "A1": {
        "sites": [(M, "        FwPos::Got(held) => {\n            let counted_ticket"),
                  (M, "        FwPos::Recorded => {\n            if o.choose")],
        "open": [
            "The first writer records its classes in the `FwPos::Got` step and confirms in `FwPos::Recorded`. "
            "'Confirms before it records' needs the two steps reordered, including the unwind choices "
            "('unwinds before recording', 'unwinds before confirming') and the DROP choice, none of which an "
            "authorised source specifies.",
            "Whether settlement's own record-then-confirm (SE) is also 'the writer' is not stated.",
        ],
    },
    "A2": {
        "sites": [(M, "fn step_adm(config: &Config, w: &mut World, obs: &mut Obs) -> bool {"),
                  (M, "    /// Actor change A2 only: the bits kept by the first step.")],
        "open": [
            "The contract states A2's semantics (two steps, two holds, the kept low 16 bits). The world already "
            "has a key field for the kept bits (`adm_kept`), but the two-step actor code is given by no "
            "authorised source; writing it is authoring a new actor program.",
        ],
    },
    "A3": {
        "sites": [(M, "            if let Action::EmptyX(_, seq) = &action {\n                w.table.h = w.table.h.max(*seq);"),
                  (M, "    /// Under actor change A3 only: H raised after X was emptied.")],
        "open": [
            "The world already has `WkPos::RaiseH` ('Under actor change A3 only'), but the transitions into it "
            "(stop raising H in `Seen`, go to `RaiseH` after `Act(EmptyX)`) are given by no authorised source.",
        ],
    },
    "A5": {
        "sites": [(M, "        Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => {")],
        "open": [
            "'Settlement claims a Conflict obligation while the store is not Healthy': whether only the Healthy "
            "conjunct is dropped from the guard, and the exact text. No authorised source gives it.",
        ],
    },
    "A6": {
        "sites": [(M, "    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;")],
        "open": [
            "'Recovery required is a flag, not a comparison of H with A' needs a new world field (part of the "
            "state key, 7.1) and rules for setting and clearing it. None is given by an authorised source.",
        ],
    },
}
