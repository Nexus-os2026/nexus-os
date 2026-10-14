"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01: the 21 Architect control definitions of mission section 6, encoded.

ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING.

Pure data: no code runs at import beyond building these literals. Each control is one or more exact edits of one
source file of Candidate C (d5964ee5); an edit is (anchor, replacement), the anchor being exact baseline text that
occurs exactly once in that file's blob at C and starts at a line start. Edits of one control are applied in order
to disjoint anchors. The patch text is generated from the resulting bytes (derive_catalog.py); nothing here is
applied to any worktree.
"""

PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"

DEFS = {}


def define(cid, section, title, path, item, props, props_text, config, edits, preserved, one_change, uncertain):
    DEFS[cid] = {"id": cid, "mission_section": section, "title": title, "file": path, "item": item,
                 "named_property": props, "named_property_text": props_text, "configuration": config,
                 "edits": [{"anchor": a, "replacement": r} for a, r in edits], "preserved": preserved,
                 "one_change": one_change, "uncertain": uncertain}


# ---- 6.1 C3a ---------------------------------------------------------------------------------------------------
define(
    "C3a", "6.1", "acknowledgement test 2 removed", PROTOCOL, "Publish::acknowledgement",
    ["P13", "P15"], "P13 or P15", "F-7",
    [("        let refusal = if set.frontier != now.frontier {\n"
      "            Some(AckRefusal::Stale)\n"
      "        } else if set.top <= set.frontier || set.count < 1 {\n",
      "        let refusal = if set.top <= set.frontier || set.count < 1 {\n")],
    ["the recheck verb (`io.recheck(set.frontier, set.top)`) and its failure arm",
     "Test 3 (`set.top <= set.frontier || set.count < 1` -> Empty)",
     "Test 4 (`set.top > set.head || set.head > now.head` -> Future)",
     "Test 5 (`now.count != set.count || now.digest != set.digest` -> Set)",
     "the refusal rollback, the insertion and the commit handling"],
    "The whole frontier-equality clause (condition and its Stale result) is removed and the chain's head becomes "
    "Test 3's condition; no other clause is touched.",
    ["`AckRefusal::Stale` is no longer constructed in production text; rustc may warn (dead_code). The crate has no "
     "deny(warnings) and C's .cargo/config.toml sets no RUSTFLAGS; whether a future build environment denies "
     "warnings is not known statically.",
     "Whether F-7's original explorer reaches a P13 or P15 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.2 C3b ---------------------------------------------------------------------------------------------------
define(
    "C3b", "6.2", "acknowledgement test 4 removed", PROTOCOL, "Publish::acknowledgement",
    ["P13", "P12"], "P13 or P12", "F-6",
    [("        } else if set.top > set.head || set.head > now.head {\n"
      "            Some(AckRefusal::Future)\n",
      "")],
    ["Test 2 (`set.frontier != now.frontier` -> Stale)",
     "Test 3 (`set.top <= set.frontier || set.count < 1` -> Empty)",
     "Test 5 (`now.count != set.count || now.digest != set.digest` -> Set)",
     "the recheck verb, the rollback, the insertion and the commit handling"],
    "The complete Test-4 clause (both comparisons and its Future result) is removed together; Test 3's clause is "
    "followed directly by Test 5's.",
    ["`AckRefusal::Future` is no longer constructed in production text (possible dead_code warning; see C3a).",
     "Whether F-6's original explorer reaches a P13 or P12 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.3 C3c ---------------------------------------------------------------------------------------------------
define(
    "C3c", "6.3", "acknowledgement test 5 removed", PROTOCOL, "Publish::acknowledgement",
    ["P12"], "P12", "F-6",
    [("        } else if now.count != set.count || now.digest != set.digest {\n"
      "            Some(AckRefusal::Set)\n",
      "")],
    ["Test 2, Test 3 and Test 4 clauses unchanged",
     "the final `else { None }` of the chain",
     "the recheck verb, the rollback, the insertion and the commit handling"],
    "The complete Test-5 clause (count and digest comparisons and its Set result) is removed together; after "
    "Test 4 the chain falls directly to None.",
    ["`AckRefusal::Set` is no longer constructed in production text (possible dead_code warning; see C3a).",
     "Whether F-6's original explorer reaches a P12 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.4 C5 ----------------------------------------------------------------------------------------------------
define(
    "C5", "6.4", "acknowledgement resolve raises frontier without reads", PROTOCOL, "Publish::resolve_ack",
    ["P13"], "P13", "F-5",
    [("    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {\n"
      "        match io.acknowledgement(set, nonce) {\n"
      "            Read::Got(None) => {\n"
      "                self.core.u = None;\n"
      "                self.core.phase = Phase::Healthy;\n"
      "                Resolve::Absent\n"
      "            }\n"
      "            Read::Got(Some(_)) => match io.frontier() {\n"
      "                Read::Got(durable) if durable >= set.top => {\n"
      "                    if set.top > self.core.frontier {\n"
      "                        self.core.frontier = set.top;\n"
      "                    }\n"
      "                    self.core.u = None;\n"
      "                    self.core.phase = Phase::Healthy;\n"
      "                    Resolve::Acknowledged(set.top)\n"
      "                }\n"
      "                Read::Got(_) => {\n"
      "                    self.core.latch(LatchCause::Inconsistent);\n"
      "                    Resolve::Inconsistent\n"
      "                }\n"
      "                Read::Failed => {\n"
      "                    self.failed();\n"
      "                    Resolve::Retained\n"
      "                }\n"
      "                Read::Corrupt => {\n"
      "                    self.core.latch(LatchCause::Corrupt);\n"
      "                    Resolve::Refused(Entry::Latched)\n"
      "                }\n"
      "            },\n"
      "            Read::Failed => {\n"
      "                self.failed();\n"
      "                Resolve::Retained\n"
      "            }\n"
      "            Read::Corrupt => {\n"
      "                self.core.latch(LatchCause::Corrupt);\n"
      "                Resolve::Refused(Entry::Latched)\n"
      "            }\n"
      "        }\n"
      "    }\n",
      "    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {\n"
      "        if set.top > self.core.frontier {\n"
      "            self.core.frontier = set.top;\n"
      "        }\n"
      "        self.core.u = None;\n"
      "        self.core.phase = Phase::Healthy;\n"
      "        Resolve::Acknowledged(set.top)\n"
      "    }\n")],
    ["the function signature, byte for byte (`nonce` and `io` stay, unused)",
     "the caller `Publish::resolve` and every trait signature",
     "the gating resolve (`Publish::resolve_gating`)"],
    "One item's body is replaced by the section-6.4 sequence (no acknowledgement read, no frontier read; raise, "
    "empty U, Healthy, Acknowledged); no branch of the healthy match survives.",
    ["`nonce` and `io` become unused parameters (rustc unused_variables warnings, not errors under C's lint "
     "configuration); `Resolve::Inconsistent` and `LatchCause::Inconsistent` may lose their only production "
     "constructions (dead_code warnings).",
     "Whether F-5's original explorer reaches a P13 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.5 C6 ----------------------------------------------------------------------------------------------------
define(
    "C6", "6.5", "classification row b ignores refusal table", PROTOCOL, "Core::classify",
    ["P6"], "P6", "F-PT",
    [("        if let Some(known) = self.known(&member.key, facts.explained) {\n"
      "            return Verdict::Refused(Reason::Conflict { member: at, known }, Some(at));\n"
      "        }\n",
      "        let known = match &self.x {\n"
      "            Some((_, Occupant::Explanation { original, cause }, stage))\n"
      "                if *original == member.key =>\n"
      "            {\n"
      "                Some(match stage {\n"
      "                    Stage::Built => Known::Cause(*cause),\n"
      "                    Stage::Committed(seq) => Known::Explained(*seq),\n"
      "                })\n"
      "            }\n"
      "            _ => facts.explained.map(Known::Explained),\n"
      "        };\n"
      "        if let Some(known) = known {\n"
      "            return Verdict::Refused(Reason::Conflict { member: at, known }, Some(at));\n"
      "        }\n")],
    ["`Core::known` unchanged (still used by lookup and by the model's step_gc)",
     "`Publish::lookup` unchanged",
     "the refusal table itself, and its insertion in rows d and e, unchanged",
     "precedence: X's explanation of this original before facts.explained",
     "row a (Present) before row b; rows c to e after it"],
    "Only classification row b's Known source changes: the classify call site evaluates X's explanation of this "
    "original, then facts.explained, without consulting self.refusals; every other caller of Core::known keeps "
    "the healthy rule.",
    ["The F-PT configuration carries A4 (blanket Uncertain) by definition; whether the original explorer reaches a "
     "P6 failure within 30,000,000 states is decided only by execution."])

# ---- 6.6 C7 ----------------------------------------------------------------------------------------------------
define(
    "C7", "6.6", "Explained clears every refusal-table entry", PROTOCOL, "Publish::explain",
    ["P6"], "P6", "F-PT",
    [("            Read::Got(Some(seq)) => {\n"
      "                self.core.refusals.remove(key);\n",
      "            Read::Got(Some(seq)) => {\n"
      "                self.core.refusals.clear();\n")],
    ["`return Explain::Explained(seq);` unchanged",
     "`Publish::settle` (X's committed path) and its `self.core.refusals.remove(&original);` unchanged",
     "every other table-removal site unchanged"],
    "In explain's Read::Got(Some(seq)) arm only, removal of the one key is replaced by clearing the whole table.",
    ["The same patch serves the required F-PT (with A4) kill run and the separate AD-1-R4 F (line 748) "
     "original-explorer F-3 comparison without A4, which is not required to kill.",
     "Whether F-PT's original explorer reaches a P6 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.7 C8 ----------------------------------------------------------------------------------------------------
define(
    "C8", "6.7", "classify and insert each member immediately", PROTOCOL, "Publish::facts",
    ["E5"], "E5", "F-B",
    [("        // Step 3: every member classified before any insert.\n"
      "        let mut pending = [false; BATCH_MAX];\n"
      "        let mut any = false;\n"
      "        let members = || batch.iter().map(FactRecord::member);\n"
      "        for (index, (member, slot)) in members().zip(pending.iter_mut()).enumerate() {\n"
      "            let at = member_index(index);\n"
      "            let facts = match io.facts(at, member) {\n"
      "                Read::Got(facts) => facts,\n"
      "                failed => {\n"
      "                    self.read_failed(failed == Read::Corrupt);\n"
      "                    self.rolled(io.rollback());\n"
      "                    return self.not_applied(Persistence::NotWritten, Reason::Storage);\n"
      "                }\n"
      "            };\n"
      "            match self\n"
      "                .core\n"
      "                .classify(at, member, &facts, members().take(index), false)\n"
      "            {\n"
      "                Verdict::Present(_) => {}\n"
      "                Verdict::Insert => {\n"
      "                    *slot = true;\n"
      "                    any = true;\n"
      "                }\n"
      "                Verdict::Refused(reason, conflict) => {\n"
      "                    self.conflict = conflict;\n"
      "                    self.rolled(io.rollback());\n"
      "                    return self.not_applied(Persistence::NotWritten, reason);\n"
      "                }\n"
      "            }\n"
      "        }\n"
      "        // Step 4.\n"
      "        if !any {\n"
      "            self.rolled(io.rollback());\n"
      "            return FactOutcome::Committed;\n"
      "        }\n"
      "        // Step 5.\n"
      "        for (index, (member, slot)) in members().zip(pending.iter()).enumerate() {\n"
      "            if !*slot {\n"
      "                continue;\n"
      "            }\n"
      "            self.issued = true;\n"
      "            let reason = match io.insert(member_index(index), member) {\n"
      "                Insert::Inserted(_) => continue,\n"
      "                Insert::Constraint => {\n"
      "                    self.core.latch(LatchCause::Constraint);\n"
      "                    Reason::Constraint\n"
      "                }\n"
      "                Insert::Failed => Reason::Storage,\n"
      "            };\n"
      "            self.failed();\n"
      "            let persistence = self.rolled(io.rollback());\n"
      "            return self.not_applied(persistence, reason);\n"
      "        }\n",
      "        let mut any_inserted = false;\n"
      "        let members = || batch.iter().map(FactRecord::member);\n"
      "        for (index, member) in members().enumerate() {\n"
      "            let at = member_index(index);\n"
      "            let facts = match io.facts(at, member) {\n"
      "                Read::Got(facts) => facts,\n"
      "                failed => {\n"
      "                    self.read_failed(failed == Read::Corrupt);\n"
      "                    self.rolled(io.rollback());\n"
      "                    return self.not_applied(Persistence::NotWritten, Reason::Storage);\n"
      "                }\n"
      "            };\n"
      "            match self\n"
      "                .core\n"
      "                .classify(at, member, &facts, members().take(index), false)\n"
      "            {\n"
      "                Verdict::Present(_) => {}\n"
      "                Verdict::Insert => {\n"
      "                    self.issued = true;\n"
      "                    any_inserted = true;\n"
      "                    let reason = match io.insert(at, member) {\n"
      "                        Insert::Inserted(_) => continue,\n"
      "                        Insert::Constraint => {\n"
      "                            self.core.latch(LatchCause::Constraint);\n"
      "                            Reason::Constraint\n"
      "                        }\n"
      "                        Insert::Failed => Reason::Storage,\n"
      "                    };\n"
      "                    self.failed();\n"
      "                    let persistence = self.rolled(io.rollback());\n"
      "                    return self.not_applied(persistence, reason);\n"
      "                }\n"
      "                Verdict::Refused(reason, conflict) => {\n"
      "                    self.conflict = conflict;\n"
      "                    self.rolled(io.rollback());\n"
      "                    return self.not_applied(Persistence::NotWritten, reason);\n"
      "                }\n"
      "            }\n"
      "        }\n"
      "        // Step 4.\n"
      "        if !any_inserted {\n"
      "            self.rolled(io.rollback());\n"
      "            return FactOutcome::Committed;\n"
      "        }\n")],
    ["Steps 0, 1 and 2 unchanged",
     "the facts read and its failure arm, byte for byte (healthy text carried into the loop)",
     "the classify call and its earlier-member input (`members().take(index)`)",
     "the insert arms: Constraint latches; Constraint and Failed each take self.failed(), exactly one rollback and "
     "the rollback-derived persistence",
     "a refusal: self.conflict from the verdict, exactly one rollback, Not written with the classification reason",
     "the no-insert path: the empty-transaction rollback and Committed",
     "Step 6 commit handling unchanged"],
    "The two-phase Step-3/Step-5 structure becomes one member-order loop: the pending array and the separate "
    "insertion loop are removed, and each member's insert follows its own classification at once. The healthy "
    "comment '// Step 3: every member classified before any insert.' and the '// Step 5.' marker, which describe "
    "the removed structure, are removed with it; no comment is added.",
    ["The facts-read failure arm is carried unchanged into the merged loop: after an earlier member was inserted it "
     "still reports Not written (the healthy arm's text), as section 6.7 specifies for a refusal. If the "
     "explorer's first violation on F-B is that arm's persistence (E4) rather than E5, the printed property would "
     "differ; only execution decides.",
     "`BATCH_MAX` keeps its other use (`well_formed`), so no new unused item.",
     "Whether F-B's original explorer reaches an E5 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.8 C11 ---------------------------------------------------------------------------------------------------
define(
    "C11", "6.8", "constraint insertion failure always Not written", PROTOCOL, "Publish::facts (Step 5)",
    ["E4"], "E4", "F-B2",
    [("            let reason = match io.insert(member_index(index), member) {\n"
      "                Insert::Inserted(_) => continue,\n"
      "                Insert::Constraint => {\n"
      "                    self.core.latch(LatchCause::Constraint);\n"
      "                    Reason::Constraint\n"
      "                }\n",
      "            let reason = match io.insert(member_index(index), member) {\n"
      "                Insert::Inserted(_) => continue,\n"
      "                Insert::Constraint => {\n"
      "                    self.core.latch(LatchCause::Constraint);\n"
      "                    self.failed();\n"
      "                    self.rolled(io.rollback());\n"
      "                    return self.not_applied(Persistence::NotWritten, Reason::Constraint);\n"
      "                }\n")],
    ["Insert::Failed keeps the healthy shared path: self.failed(), one rollback, rollback-derived persistence",
     "the gating insert constraint (Publish::gating) unchanged",
     "the acknowledgement insert constraint (Publish::acknowledgement) unchanged",
     "the cell insert constraint (Publish::cell) unchanged",
     "exactly one rollback on the Constraint path (the arm returns before the shared rollback)"],
    "Only the fact append's Step-5 Insert::Constraint arm changes: it latches, fails, issues its one rollback, "
    "ignores the rollback's answer and returns Not written with Reason::Constraint.",
    ["Whether F-B2's original explorer reaches an E4 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.9 C15 ---------------------------------------------------------------------------------------------------
define(
    "C15", "6.9", "occupancy number ignored", PROTOCOL, "Core::names",
    ["E8"], "E8", "F-R",
    [("        Arc::ptr_eq(&token.ledger, &self.shared)\n"
      "            && token.cell == cell\n"
      "            && self.occupancy(cell) == Some(token.occupancy)\n",
      "        Arc::ptr_eq(&token.ledger, &self.shared) && token.cell == cell\n")],
    ["the same-ledger-allocation condition (`Arc::ptr_eq(&token.ledger, &self.shared)`)",
     "the cell-equality condition (`token.cell == cell`)",
     "no `is_some()` occupancy test is introduced: a token naming an empty cell passes if ledger and cell match",
     "every caller of Core::names (resolve, the cell appends, empty) unchanged"],
    "Only the occupancy-equality conjunct of Core::names is removed; the two remaining conjuncts are rejoined on "
    "one line as rustfmt requires.",
    ["AD-1-R5 governs C15's run: original full_fr, 30,000,000; its restored side may cite the adopted F-R proof "
     "discharge only after verified restoration. Whether the mutant fails E8 within the ceiling is decided only by "
     "execution."])

# ---- 6.10 C16 --------------------------------------------------------------------------------------------------
define(
    "C16", "6.10", "Counted drop lowers unapplied", PROTOCOL, "Drop for Ticket",
    ["E3"], "E3", "F-D",
    [("            TicketState::Counted(_) => {\n"
      "                exchange(&self.shared, 0, |word| word | HANDOVER_FAULT);\n",
      "            TicketState::Counted(_) => {\n"
      "                exchange(&self.shared, 0, |word| {\n"
      "                    count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false)\n"
      "                });\n")],
    ["HANDOVER_FAULT is still set",
     "`live` is not lowered (no LIVE_SHIFT step)",
     "the Armed and Settled arms of Drop for Ticket unchanged",
     "Ticket::confirm (the full Counted confirm path) unchanged and not called"],
    "Only the Counted arm of Drop for Ticket changes: its exchange closure becomes "
    "count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false).",
    ["Whether F-D's original explorer reaches an E3 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.11 C17 --------------------------------------------------------------------------------------------------
define(
    "C17", "6.11", "unwind guard reports Not written after insert", PROTOCOL, "Drop for Publish",
    ["E4"], "E4", "F-B2",
    [("                let persistence = if self.issued && self.conflict.is_none() {\n"
      "                    Persistence::Uncertain\n"
      "                } else {\n"
      "                    Persistence::NotWritten\n"
      "                };\n",
      "                let persistence = Persistence::NotWritten;\n")],
    ["`self.core.phase = Phase::NeedBarrier;` on unwind",
     "self.conflict and conflict_member unchanged",
     "self.issued unchanged",
     "`self.keep = self.issued;` unchanged",
     "U retention and exit marking unchanged",
     "the publication (exchange) and the ticket-state assignment unchanged"],
    "Only the unwind persistence calculation changes: the Counted ticket created during unwind always carries "
    "Persistence::NotWritten.",
    ["Whether F-B2's original explorer reaches an E4 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.12 C18a -------------------------------------------------------------------------------------------------
define(
    "C18a", "6.12", "classification-refused gating attempt keeps U", PROTOCOL, "Publish::gating",
    ["E7"], "E7", "F-G",
    [("            Verdict::Refused(reason, _) => {\n"
      "                self.rolled(io.rollback());\n"
      "                return refused(Persistence::NotWritten, reason);\n",
      "            Verdict::Refused(reason, _) => {\n"
      "                self.keep = true;\n"
      "                self.rolled(io.rollback());\n"
      "                return refused(Persistence::NotWritten, reason);\n")],
    ["the rollback and persistence = NotWritten",
     "the refusal reason",
     "the open, facts-read, insert and commit failure paths of gating (no keep added there)",
     "the entry refusals before U is placed",
     "the guard's normal exit marking (Drop for Publish)"],
    "Only the classification-refused arm of gating sets `self.keep = true;` before its rollback and return.",
    ["The guard then marks the kept gating occupant ReturnedUncertain (healthy Drop for Publish behaviour). Whether "
     "F-G's original explorer reaches an E7 failure within 30,000,000 states is decided only by execution."])

# ---- 6.13 C18b -------------------------------------------------------------------------------------------------
define(
    "C18b", "6.13", "gating new cause also enters refusal table", PROTOCOL, "Core::classify (gating branch)",
    ["E7"], "E7", "F-G",
    [("        if gating {\n"
      "            if self.x.is_none() {\n"
      "                self.explanation(member.key, cause);\n"
      "            }\n"
      "            return Verdict::Refused(conflict, Some(at));\n"
      "        }\n",
      "        if gating {\n"
      "            if self.x.is_none() {\n"
      "                self.explanation(member.key, cause);\n"
      "            }\n"
      "            self.refusals.insert(member.key, cause);\n"
      "            return Verdict::Refused(conflict, Some(at));\n"
      "        }\n")],
    ["X's explanation is still built when X is empty",
     "the same conflict is returned",
     "no fall-through to the fact-classification branch; no table-full check or latch is applied",
     "already-known conflict handling (row b) unchanged",
     "the gating correction test at the top of classify unchanged"],
    "Only the gating new-cause branch adds one table insertion (member.key -> cause) after the X explanation and "
    "before its return.",
    ["Whether F-G's original explorer reaches an E7 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.14 C20 --------------------------------------------------------------------------------------------------
define(
    "C20", "6.14", "present X occupant is not marked committed", PROTOCOL, "Publish::cell",
    ["L1"], "L1", "F-2",
    [("            Read::Got(Some(seq)) => {\n"
      "                self.rolled(io.rollback());\n"
      "                self.settle(cell, seq);\n"
      "                return CellAppend::Committed(seq);\n",
      "            Read::Got(Some(seq)) => {\n"
      "                self.rolled(io.rollback());\n"
      "                if cell == Cell::C {\n"
      "                    self.settle(cell, seq);\n"
      "                }\n"
      "                return CellAppend::Committed(seq);\n")],
    ["the rollback",
     "C's present path: settle (C emptied) unchanged",
     "the return CellAppend::Committed(seq) for both cells",
     "the inserted-and-Durable path (`Commit::Durable => { self.settle(cell, seq); ... }`) unchanged",
     "Publish::settle itself unchanged"],
    "Only the present-occupant arm changes: settle is called only for C, so a present X occupant is neither marked "
    "Committed nor has the original's table entry removed.",
    ["L1 is evaluated only after a complete search (AD-1-R4 Q); whether F-2's original explorer completes and "
     "reports L1 within 30,000,000 states is decided only by execution."])

# ---- 6.15 C23 --------------------------------------------------------------------------------------------------
define(
    "C23", "6.15", "X publication bit omitted", PROTOCOL, "Core::flags",
    ["E1"], "E1", "F-2",
    [("            | bit(self.x.is_some(), X_OCCUPIED)\n", "")],
    ["the phase and session terms; the LATCHED, U_OCCUPIED and C_OCCUPIED bits",
     "the model's expected_flags unchanged",
     "the word layout constants unchanged (X_OCCUPIED keeps its use in HealthView::x_occupied)"],
    "Only the X bit term is removed from the returned flags expression.",
    ["Whether F-2's original explorer reaches an E1 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.16 C26 --------------------------------------------------------------------------------------------------
define(
    "C26", "6.16", "successful barrier clears latch", PROTOCOL, "Publish::barrier",
    ["E9"], "E9", "F-4",
    [("        if !io.run() {\n"
      "            return BarrierOutcome::Failed;\n"
      "        }\n",
      "        if !io.run() {\n"
      "            return BarrierOutcome::Failed;\n"
      "        }\n"
      "        self.core.latch = None;\n")],
    ["OtherLedger, NotNeeded and the failed barrier return before the clearing",
     "the healthy phase assignment and `BarrierOutcome::Passed` unchanged"],
    "One statement, `self.core.latch = None;`, is inserted after io.run() returns true and before the phase "
    "assignment.",
    ["Whether F-4's original explorer reaches an E9 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.17 A1 ---------------------------------------------------------------------------------------------------
define(
    "A1", "6.17", "first writer confirms before recording", MODEL, "step_fw",
    ["P1"], "P1", "F-1",
    [("        FwPos::Got(held) => {\n"
      "            let counted_ticket = matches!(held, Held::Counted(_));\n"
      "            let options: &[bool] = if config.drop && counted_ticket {\n"
      "                &[false, true, false]\n"
      "            } else {\n"
      "                &[false, true]\n"
      "            };\n"
      "            match o.choose(options) {\n"
      "                0 => {\n"
      "                    match held {\n"
      "                        Held::Committed => {\n"
      "                            for &f in &members {\n"
      "                                set_class(&mut w.table, f, Class::Paid);\n"
      "                            }\n"
      "                        }\n"
      "                        Held::Counted(counted) => record_counted(w, &members, counted),\n"
      "                        Held::Rejected => {\n"
      "                            for &f in &members {\n"
      "                                set_class(&mut w.table, f, Class::Uncertain);\n"
      "                                w.ghost.unrecorded &= !bit(f);\n"
      "                            }\n"
      "                        }\n"
      "                    }\n"
      "                    w.fw[i].pos = FwPos::Recorded;\n"
      "                    obs.note(|| format!(\"[T] records {held:?}\"));\n"
      "                }\n",
      "        FwPos::Got(held) if w.fw[i].ticket.is_none() => {\n"
      "            match held {\n"
      "                Held::Committed => {\n"
      "                    for &f in &members {\n"
      "                        set_class(&mut w.table, f, Class::Paid);\n"
      "                    }\n"
      "                }\n"
      "                Held::Counted(counted) => record_counted(w, &members, counted),\n"
      "                Held::Rejected => {\n"
      "                    for &f in &members {\n"
      "                        set_class(&mut w.table, f, Class::Uncertain);\n"
      "                        w.ghost.unrecorded &= !bit(f);\n"
      "                    }\n"
      "                }\n"
      "            }\n"
      "            w.fw[i].pos = FwPos::Done;\n"
      "            obs.note(|| format!(\"[T] records {held:?}\"));\n"
      "        }\n"
      "        FwPos::Got(held) => {\n"
      "            let counted_ticket = matches!(held, Held::Counted(_));\n"
      "            let options: &[bool] = if config.drop && counted_ticket {\n"
      "                &[false, true, false]\n"
      "            } else {\n"
      "                &[false, true]\n"
      "            };\n"
      "            match o.choose(options) {\n"
      "                0 => {\n"
      "                    obs.ticket_steps += 1;\n"
      "                    w.fw[i]\n"
      "                        .ticket\n"
      "                        .take()\n"
      "                        .expect(\"a writer that has its result holds its ticket\")\n"
      "                        .confirm();\n"
      "                    obs.note(|| \"confirm\".into());\n"
      "                }\n")],
    ["settlement (step_se) unchanged, including its own record-then-confirm",
     "the Oracle option structure of FwPos::Got with a ticket ([false, true] or, with DROP, [false, true, false])",
     "option 1 (unwind before recording -> Dropping) and the DROP choice unchanged",
     "the recording rules (Committed -> Paid; Counted -> record_counted; Rejected -> Uncertain and the unrecorded "
     "bit cleared) carried verbatim into the post-confirm step",
     "FwPos::Recorded stays in the enum and in its healthy branch, unreachable from option 0",
     "no new enum variant; the two steps are told apart by ticket presence, which is in the state key",
     "option 0's confirm keeps the healthy confirm's own accounting (obs.ticket_steps += 1, as in FwPos::Recorded), "
     "so E2 still accounts for the ticket's exchange"],
    "Only the first writer's FwPos::Got step changes: option 0 takes and confirms the ticket and leaves the "
    "position Got(held); a new guarded arm (Got with no ticket) records exactly as healthy option 0 did and moves "
    "to Done, consuming no Oracle choice.",
    ["No healthy path reaches FwPos::Got without a ticket (the Armed step always puts the ticket back), so the new "
     "guarded arm is reached only through the mutant's option 0. This is a static reading; it is not executed.",
     "The guarded arm borrows w.fw[i].ticket in the match guard while matching the Copy position; this is believed "
     "to type-check (the healthy arm already mutates w.fw[i] in its body) but is SYNTAX_CHECKED_NOT_TYPECHECKED.",
     "Whether F-1's original explorer reaches a P1 failure within 60,000,000 states is decided only by "
     "execution."])

# ---- 6.18 A2 ---------------------------------------------------------------------------------------------------
define(
    "A2", "6.18", "split-view admission", MODEL, "step_adm",
    ["P1"], "P1", "F-1",
    [("    if w.adm == 0 {\n"
      "        return false;\n"
      "    }\n"
      "    obs.views += 1;\n"
      "    let view = w.health.view();\n"
      "    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;\n"
      "    w.adm -= 1;\n",
      "    let Some(kept) = w.adm_kept else {\n"
      "        if w.adm == 0 {\n"
      "            return false;\n"
      "        }\n"
      "        obs.views += 1;\n"
      "        let first = w.health.view();\n"
      "        w.adm_kept = Some(first.word & FLAGS);\n"
      "        obs.note(|| \"[T] admission step one: the low 16 bits kept\".into());\n"
      "        return true;\n"
      "    };\n"
      "    obs.views += 1;\n"
      "    let second = w.health.view();\n"
      "    let view = HealthView {\n"
      "        word: kept | with_field(0, UNAPPLIED_SHIFT, field(second.word, UNAPPLIED_SHIFT)),\n"
      "    };\n"
      "    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;\n"
      "    w.adm_kept = None;\n"
      "    w.adm -= 1;\n")],
    ["the existing World.adm_kept field and its key encoding; no new world field",
     "step one: disabled when adm == 0; one view (E2 counts it); keeps only view.word & FLAGS; adm not decremented; "
     "no admission branch; no admission property; enabled",
     "step two: one view (E2 counts it); only its unapplied field used; the decision word's low 16 bits are the kept "
     "bits and its other fields zero",
     "the decision line text, `view.admits() && w.table.debt == 0 && w.table.h <= w.table.a`, unchanged and read "
     "from the current table",
     "adm_kept cleared and adm decremented exactly once in step two",
     "the healthy admission branch, note and admission_properties unchanged after the decision"],
    "Only step_adm changes: it becomes the two-step split admission of section 6.18, keyed by the existing "
    "adm_kept field, with the healthy decision expression applied to the split HealthView.",
    ["Step one adds a trace note (obs.note) and no branch: notes affect only trace text, never a verdict or the "
     "branch set.",
     "Experimental quotient/symmetry code reads adm_kept (e.g. asserting it is None); those explorers are not "
     "used for A2's original-explorer run on F-1. Static reading only.",
     "Whether F-1's original explorer reaches a P1 failure within 60,000,000 states is decided only by "
     "execution."])

# ---- 6.19 A3 ---------------------------------------------------------------------------------------------------
define(
    "A3", "6.19", "EmptyX before H", MODEL, "step_wk",
    ["P12"], "P12", "F-5",
    [("            w.table.a = w.table.a.max(frontier);\n"
      "            if let Action::EmptyX(_, seq) = &action {\n"
      "                w.table.h = w.table.h.max(*seq);\n"
      "            }\n",
      "            w.table.a = w.table.a.max(frontier);\n"),
     ("        WkPos::Act(action) => {\n"
      "            worker_op(config, w, &action, o, obs);\n"
      "            if config.replays > 0 && !matches!(action, Action::Barrier | Action::Nothing) {\n"
      "                w.wk.last = Some(action);\n"
      "            }\n"
      "            w.wk.pos = WkPos::Idle;\n"
      "        }\n",
      "        WkPos::Act(action) => {\n"
      "            worker_op(config, w, &action, o, obs);\n"
      "            let next = match &action {\n"
      "                Action::EmptyX(_, seq) => WkPos::RaiseH(*seq),\n"
      "                _ => WkPos::Idle,\n"
      "            };\n"
      "            if config.replays > 0 && !matches!(action, Action::Barrier | Action::Nothing) {\n"
      "                w.wk.last = Some(action);\n"
      "            }\n"
      "            w.wk.pos = next;\n"
      "        }\n")],
    ["WkPos::Seen keeps the A update from the agenda frontier and proceeds to Act(action) for EmptyX as for "
     "another real action",
     "worker_op runs first in WkPos::Act; the replay-history save is unchanged",
     "WkPos::RaiseH's existing behaviour (H = max(H, seq); Idle) unchanged",
     "no new enum variant",
     "the Seen note text is unchanged (it is trace text only)"],
    "Only step_wk changes: H is no longer raised in Seen for EmptyX; after the worker's EmptyX operation the "
    "worker moves to the existing WkPos::RaiseH(seq), which raises H in a later step. The intermediate state is "
    "the named fault.",
    ["Whether F-5's original explorer reaches a P12 failure within 30,000,000 states is decided only by "
     "execution."])

# ---- 6.20 A5 ---------------------------------------------------------------------------------------------------
define(
    "A5", "6.20", "Conflict claimed without Healthy", MODEL, "claimable",
    ["L1"], "L1", "F-2",
    [("        Class::Conflict if view.phase() == Phase::Healthy && !view.x_occupied() => {\n"
      "            Some(Claim::Explain)\n"
      "        }\n",
      "        Class::Conflict if !view.x_occupied() => Some(Claim::Explain),\n")],
    ["the handover-fault and latch rejection before the match",
     "the Refused arm (Healthy -> Append) and the Uncertain arm (not NeedBarrier -> Lookup)",
     "the X-empty requirement of the Conflict arm",
     "the claim type Explain"],
    "Only the Healthy-phase conjunct of the Conflict arm's guard is removed; rustfmt then requires the arm's "
    "single-expression block to be written on one line.",
    ["L1 is evaluated only after a complete search (AD-1-R4 Q); whether F-2's original explorer completes and "
     "reports L1 within 30,000,000 states is decided only by execution."])

# ---- 6.21 A6 ---------------------------------------------------------------------------------------------------
define(
    "A6", "6.21", "recovery-required event flag", MODEL,
    "Table; initial; encode; decode; step_wk (Seen, RaiseH); step_ack (Applying); step_adm",
    ["P12", "P15"], "P12 or P15", "F-5",
    [("    /// The layer's copy of the acknowledgement frontier.\n"
      "    a: Seq,\n"
      "}\n",
      "    /// The layer's copy of the acknowledgement frontier.\n"
      "    a: Seq,\n"
      "    recovery_required: bool,\n"
      "}\n"),
     ("            h: 0,\n"
      "            a: 0,\n"
      "        },\n",
      "            h: 0,\n"
      "            a: 0,\n"
      "            recovery_required: false,\n"
      "        },\n"),
     ("    e.u(w.table.h);\n"
      "    e.u(w.table.a);\n",
      "    e.u(w.table.h);\n"
      "    e.u(w.table.a);\n"
      "    e.b(w.table.recovery_required);\n"),
     ("        h: d.u(),\n"
      "        a: d.u(),\n"
      "    };\n",
      "        h: d.u(),\n"
      "        a: d.u(),\n"
      "        recovery_required: d.b(),\n"
      "    };\n"),
     ("            w.table.a = w.table.a.max(frontier);\n"
      "            if let Action::EmptyX(_, seq) = &action {\n"
      "                w.table.h = w.table.h.max(*seq);\n"
      "            }\n",
      "            if frontier > w.table.a {\n"
      "                w.table.a = frontier;\n"
      "                w.table.recovery_required = false;\n"
      "            }\n"
      "            if let Action::EmptyX(_, seq) = &action {\n"
      "                if *seq > w.table.h {\n"
      "                    w.table.h = *seq;\n"
      "                    w.table.recovery_required = true;\n"
      "                }\n"
      "            }\n"),
     ("        WkPos::RaiseH(seq) => {\n"
      "            w.table.h = w.table.h.max(seq);\n",
      "        WkPos::RaiseH(seq) => {\n"
      "            if seq > w.table.h {\n"
      "                w.table.h = seq;\n"
      "                w.table.recovery_required = true;\n"
      "            }\n"),
     ("        AckPos::Applying(n) => {\n"
      "            w.table.a = w.table.a.max(n);\n",
      "        AckPos::Applying(n) => {\n"
      "            if n > w.table.a {\n"
      "                w.table.a = n;\n"
      "                w.table.recovery_required = false;\n"
      "            }\n"),
     ("    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;\n",
      "    let admitted = view.admits() && w.table.debt == 0 && !w.table.recovery_required;\n")],
    ["numeric H and A keep their values (assignment only on an actual increase, the same result as max)",
     "the flag is in the state key: encoded after A and decoded at the matching place",
     "every Table construction (initial, decode) carries the flag",
     "the flag is set only where H actually increases (WkPos::Seen EmptyX, WkPos::RaiseH) and cleared only where A "
     "actually increases (WkPos::Seen agenda frontier, AckPos::Applying); no comparison of A with H",
     "only step_adm's `w.table.h <= w.table.a` conjunct is replaced; every other admission condition is unchanged",
     "admission_properties (P12, P15) unchanged: they read the ledger rows and the durable frontier, not the flag",
     "the experimental negative-case admissions and the property code keep `h <= a` where they have it",
     "no production source is changed"],
    "One actor/model mutation: 'recovery required' becomes an event flag kept in the test's table (set on an H "
    "increase, cleared on an A increase), and admission reads the flag instead of comparing H with A. Its eight "
    "edits are the field, its two constructions, its encoding and decoding, its two setting sites, its two "
    "clearing sites (one edit covers the Seen A and H updates) and the admission condition.",
    ["KEY_FIELDS (the receipt's printed list of key fields) is not changed; it still says the table holds "
     "'classes, debt counter, H, A'. Section 6.21 does not ask for it.",
     "Experimental quotient code that relies on the key's byte layout after A is not used by A6's original-explorer "
     "run; static reading only.",
     "Whether F-5's original explorer reaches a P12 or P15 failure within 30,000,000 states is decided only by "
     "execution."])

ORDER = ["C3a", "C3b", "C3c", "C5", "C6", "C7", "C8", "C11", "C15", "C16", "C17", "C18a", "C18b", "C20", "C23",
         "C26", "A1", "A2", "A3", "A5", "A6"]
assert sorted(ORDER) == sorted(DEFS), "ORDER and DEFS disagree"
