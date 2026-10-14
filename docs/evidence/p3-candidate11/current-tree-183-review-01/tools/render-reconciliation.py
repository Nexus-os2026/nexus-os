#!/usr/bin/env python3
"""Render RECONCILIATION-22.md from reconcile-22.py's raw output and analysis-22.json.

Fails closed unless the analysis covers exactly the 22 pairs the facts list and
every label is an executor-analysis label. Writes only --out.
"""
import argparse
import json
import sys

LABELS = {'EXECUTOR_ANALYSIS_MATCH', 'EXECUTOR_ANALYSIS_DIFFERENCE'}
ORDER = ['M32', 'M78', 'M79', 'M80', 'M87', 'M89', 'M112', 'M113', 'M141', 'M143', 'M153',
         'M177', 'M178', 'M179', 'M180', 'M18', 'M19', 'M39', 'M84', 'M05', 'M45', 'M129']


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--facts', required=True)
    parser.add_argument('--analysis', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    raw = open(args.facts, encoding='utf-8').read()
    head, rest = raw.split('\n', 1)
    body, tail = rest.rsplit('PAIRS ', 1)
    facts = {r['control']: r for r in json.loads(body)}
    if tail.strip() != '22' or sorted(facts) != sorted(ORDER):
        sys.exit('facts do not list exactly the 22 pairs')
    analysis = json.load(open(args.analysis, encoding='utf-8'))
    entries = analysis['entries']
    if sorted(entries) != sorted(ORDER):
        sys.exit('analysis does not cover exactly the 22 pairs')
    if any(e['label'] not in LABELS for e in entries.values()):
        sys.exit('analysis uses a label outside %s' % sorted(LABELS))

    out = []
    w = out.append
    w('# The 22 stale entries: reconciliation records')
    w('')
    w('Mission P3-C11-DR01-D2A1-CTM01-CANDIDATE-BASELINE-01. Contract basis: AD-1-R4 H.4.')
    w('')
    w('**Every entry: ARCHITECT_REVIEW_PENDING.** The labels below are the executor\'s static '
      'analysis (EXECUTOR_ANALYSIS_MATCH or EXECUTOR_ANALYSIS_DIFFERENCE). None is a SAME PROPERTY '
      'finding, and none is acceptance: that review belongs to the Sole Architect. No build, test '
      'or mutation was run. A string is not authority.')
    w('')
    w('Facts source: `receipts/09-reconcile-22-facts.txt` (raw output of `tools/reconcile-22.py`), '
      'whose first line is:')
    w('')
    w('```text')
    w(head)
    w('```')
    w('')
    w('Terms. **Recorded**: the source sha256 the historical manifest records for the file. **H**: '
      'the last commit at which the file still has the recorded hash, the parent of the **transition '
      'commit**, the first commit (path-limited history of the validated commit) after which it no '
      'longer does; derived by `reconcile-22.py`, which asserts that H holds the recorded hash. '
      '**First-known stale (existing evidence)**: the receipt named in the facts header, which sampled '
      'five commits; it is cited, not re-derived. **Now**: the validated commit. Lines are 1-based '
      'start lines of the anchor. "Context" compares the anchor\'s +/-20 lines at H and now. Function '
      'identity is by name and rustfmt layout (from the `fn` line to the first later line that is '
      'its indentation followed by `}`).')
    w('')
    counts = {}
    for e in entries.values():
        counts[e['label']] = counts.get(e['label'], 0) + 1
    w('## Summary')
    w('')
    w('| Control | Set | File | Transition | Anchor line H → now | Definition byte-equal | '
      'Expected tests byte-identical | Executor analysis |')
    w('|---|---|---|---|---|---|---|---|')
    for cid in ORDER:
        f = facts[cid]
        a = f['anchors']
        lines = '; '.join('%s → %s' % (x['lines_at_H'], x['lines_now']) for x in a)
        tests_same = all(t['test_fn_text_identical_H_to_now'] for t in f['expected_tests'])
        w('| %s | %s | `%s` | `%s` | %s | %s | %s | %s |' % (
            cid, f['set'], f['file'], f['derived_transition_commit'][:12], lines,
            'yes' if all(f['definition_byte_equal'].values()) else '**NO**',
            'yes' if tests_same else '**NO**', entries[cid]['label']))
    w('')
    w('Labels: ' + ', '.join('%s %d' % (k, v) for k, v in sorted(counts.items())) + '. '
      'Review of all 22: ARCHITECT_REVIEW_PENDING.')
    w('')
    common = analysis['_p3_common']
    for cid in ORDER:
        f = facts[cid]
        e = entries[cid]
        w('## %s — %s' % (cid, f['meaning']))
        w('')
        w('| Field | Value |')
        w('|---|---|')
        w('| Set | %s |' % f['set'])
        w('| Category | %s |' % f['category'])
        w('| Target file | `%s` |' % f['file'])
        w('| Recorded (historical) sha256 | `%s` |' % f['historical_recorded_sha256'])
        w('| Current sha256 | `%s` |' % f['current_sha256'])
        w('| Current git blob | `%s` |' % f['current_git_blob'])
        w('| First-known stale (existing evidence) | %s |' % f['first_known_stale_from_existing_evidence'])
        w('| Transition commit (derived) | `%s` |' % f['derived_transition_commit'])
        w('| H (derived; holds the recorded hash) | `%s` |' % f['derived_historical_commit_H'])
        w('| Path history (recorded hash or other, at each change) | %s |'
          % ', '.join('`%s`' % s for s in f['path_history_states']))
        w('| File diff H → now | %s |' % f['file_diff_H_to_current'])
        eq = f['definition_byte_equal']
        w('| Definition byte-equal to historical | %s |' % ', '.join(
            '%s: %s' % (k, 'yes' if v else 'NO') for k, v in eq.items()))
        for x in f['anchors']:
            w('| Anchor (edit %d) | expected count %d; count at H %d, lines %s; count now %d, lines %s; '
              'cardinality %s |' % (x['edit'], x['expected_count'], x['count_at_H'], x['lines_at_H'],
                                    x['count_now'], x['lines_now'],
                                    'matches' if x['count_now'] == x['expected_count'] else 'MISMATCH'))
            w('| Anchor (edit %d) surroundings | enclosing function now `%s`, at H `%s`; text identical: %s; '
              '+/-20-line context identical: %s; replacement %s |' % (
                  x['edit'], x['enclosing_fn_now'], x['enclosing_fn_at_H'],
                  {True: 'yes', False: 'no', None: 'n/a'}[x['enclosing_fn_text_identical']],
                  'yes' if x['context_pm20_identical'] else 'no',
                  'is a deletion' if x['replacement_is_deletion']
                  else 'already occurs %d time(s) now' % x['replacement_occurrences_now']))
        for t in f['expected_tests']:
            w('| Expected failing test | `%s`: selected by the command\'s filter: %s; defined in %s '
              '(at H: %s); function text identical H → now: %s |' % (
                  t['test'], 'yes' if t['selected_by_command_filter'] else 'NO',
                  ', '.join('`%s`' % p for p in t['defined_in_now']),
                  ', '.join('`%s`' % p for p in t['defined_in_at_H']),
                  {True: 'yes', False: 'NO', None: 'n/a'}[t['test_fn_text_identical_H_to_now']]))
        w('| Executor analysis | **%s** |' % e['label'])
        w('| Review | **ARCHITECT_REVIEW_PENDING** |')
        w('')
        w(e['note'].replace('P3_COMMON', common))
        w('')
    data = '\n'.join(out) + '\n'
    with open(args.out, 'w', encoding='utf-8') as handle:
        handle.write(data)
    print('RENDERED %d entries; labels %s' % (len(ORDER), counts))


if __name__ == '__main__':
    main()
