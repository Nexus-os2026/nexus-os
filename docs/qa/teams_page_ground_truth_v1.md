# Builder Teams page — ground truth bugs
# Hand-documented by Suresh, April 7 2026
# Written BEFORE nexus-ui-repair v0.1 first run. Sealed reference.
# Do not edit after the scout's first run on Teams — this file is the
# tiebreaker for false-positive / false-negative measurement.

## Confirmed bugs

GT-001: Variant dropdown shows empty list
  Where: variant selector in the team detail panel
  Symptom: dropdown opens, no items
  Expected: list of variants for the selected team
  Hypothesis (don't share with scout): variants query returns [] due to
    missing team_id filter; check VariantSelector.tsx and builder/variants.rs

GT-002: "Edit team" button does nothing
  Where: row action on each team in the team list
  Symptom: click → no modal, no console error, no Tauri command
  Expected: edit modal opens populated with team fields
  Hypothesis: onClick prop not bound; check TeamsPanel.tsx render loop

## Additional bugs (fill in from memory — open Builder Teams in Nexus OS if it helps)

GT-003: [bug name]
  Where:
  Symptom:
  Expected:
  Hypothesis:

GT-004: [bug name]
  Where:
  Symptom:
  Expected:
  Hypothesis:

GT-005: [bug name]
  Where:
  Symptom:
  Expected:
  Hypothesis: