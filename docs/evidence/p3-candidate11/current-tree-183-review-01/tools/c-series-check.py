#!/usr/bin/env python3
"""Mission section 9: does the historical 183-control manifest define C9, C4 or C25
(or any D2A-1 contract 7.5 control) differently from the adopted AD-1-R4?

Read only. Checks, at --commit: (1) the 183 control ids; (2) whether any control
edits a file of the D2A-1 ledger (crates/nexus-governed-control/src/ledger*); (3)
whether any control's text contains the C-series anchors or identifiers named in
AD-1-R4 G.2; (4) that AD-1-R4 G.2 states C4, C9 and C25 as the mission restates
them. Exit 0 when no 183-control definition can conflict with an adopted C-series
definition; exit 3 (CONTROL_DEFINITION_RECONCILIATION_REQUIRED) otherwise.
"""
import argparse
import hashlib
import json
import re
import subprocess
import sys

HIST_PATH = 'docs/evidence/p3-candidate10/original-183/manifest.json'
HIST_SHA256 = '2c4adc84121c1efbbfe84aa4422444407c4d1eb4b556fbf3a0ab8e22d4782e14'
R4_PATH = ('docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/'
           'P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md')
R4_SHA256 = '00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693'
LEDGER = 'crates/nexus-governed-control/src/ledger'
# Identifiers and anchor fragments of the C-series definitions in AD-1-R4 G.2.
TERMS = ['ledger/', 'protocol.rs', 'Rollback::Unknown', 'Persistence::Uncertain',
         'Persistence::NotWritten', 'fn acknowledgement', 'AckOutcome', 'AckRefusal',
         'GatingOutcome', 'Commit::Unknown', 'self.keep', 'count_step', 'UNAPPLIED_SHIFT',
         'resolve_gating', 'Publish::', 'Resolve::Corrected', 'self.rolled(', 'LatchCause']
IDS = re.compile(r'\bC(?:[1-9]|1[0-9]|2[0-9])\b')
# AD-1-R4 G.2 sentences that state the adopted C4, C9 and C25 definitions.
G2 = {
    'C9': ['inside the arm `Rollback::Unknown`, `self.failed();` is kept and only',
           '`Persistence::Uncertain` (line 1294) is replaced with `Persistence::NotWritten`'],
    'C4': ['the complete removal of Test 1\'s five checks, exactly as the authenticated experimental patch',
           'Both P4 AND P8 remain required'],
    'C25': ['`self.failed();`, `self.rolled(io.rollback());` and `self.keep = true;`, and replace ONLY',
            '`return GatingOutcome::Committed(seq);`'],
}


def show(repo, commit, path):
    return subprocess.run(['git', '-C', repo, 'cat-file', 'blob', '%s:%s' % (commit, path)],
                          capture_output=True, check=True).stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--repo', required=True)
    parser.add_argument('--commit', required=True)
    args = parser.parse_args()
    hist_raw = show(args.repo, args.commit, HIST_PATH)
    r4 = show(args.repo, args.commit, R4_PATH)
    assert hashlib.sha256(hist_raw).hexdigest() == HIST_SHA256
    assert hashlib.sha256(r4).hexdigest() == R4_SHA256
    r4 = r4.decode('utf-8')
    controls = json.loads(hist_raw)['controls']
    ids = [c['id'] for c in controls]
    print('historical manifest: %d controls, ids %s..%s; all M-numbered: %s'
          % (len(ids), ids[0], ids[-1], all(re.fullmatch(r'M\d{2,3}', i) for i in ids)))
    ledger_targets = sorted({(c['id'], e['file']) for c in controls for e in c['edits']
                             if e['file'].startswith(LEDGER)})
    print('controls editing a ledger file: %s' % (ledger_targets or 'none'))
    term_hits = []
    id_hits = []
    for c in controls:
        text = json.dumps(c, ensure_ascii=False)
        for t in TERMS:
            if t in text:
                term_hits.append((c['id'], t))
        for m in IDS.findall(text):
            id_hits.append((c['id'], m))
    print('controls containing a C-series anchor or identifier term: %s' % (term_hits or 'none'))
    print('controls mentioning an identifier C1..C29: %s' % (id_hits or 'none'))
    g2_ok = True
    for cid, sentences in G2.items():
        for s in sentences:
            found = s in r4
            g2_ok &= found
            print('AD-1-R4 states %s: %r: %s' % (cid, s[:70], 'found' if found else 'NOT FOUND'))
    conflict = bool(ledger_targets or term_hits or id_hits) or not ids or not g2_ok
    if conflict:
        print('RESULT CONTROL_DEFINITION_RECONCILIATION_REQUIRED')
        return 3
    print('RESULT NO CONFLICT: the historical 183-control manifest defines no C-series (D2A-1 '
          'contract 7.5) control; none of its controls edits the ledger or carries a C-series '
          'anchor or identifier. C1, C4, C9, C10, C24 and C25 are defined only by AD-1-R4 G.2 '
          '(C10 and C24 by the sealed controls_defs.py it quotes), outside the 183.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
