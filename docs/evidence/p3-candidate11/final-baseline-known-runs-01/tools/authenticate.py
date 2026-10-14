#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01, mission section 3: authenticate Candidate C, its build inputs, the
proof evidence and the refs, read-only, before any worktree or target directory exists.

  authenticate.py <repo>        prints one line per check; exit 0 only if every check holds

Every read is a git object read (`cat-file`, `rev-parse`, `ls-tree`), `git ls-remote`, or a sha256 of a sealed file
outside the repository. Nothing is written to the repository.
"""
import hashlib
import os
import subprocess
import sys

C = 'd5964ee5326c2733177307db6f507328023d361a'
C_TREE = '51bac5fd335999cc995c0ab9f582a30244973ef0'
C_PARENT = '71264aa81a40b86189783facc8437e0427887c16'
PR = '0622a4d042b1dfd3160f06fc5d333e131e2a60c8'          # proof-readiness evidence commit
PR_TREE = 'a512aa45a2daf385081b55e16353f6263b95665d'
PR_PARENT = '066a322a9182fc978e3f5dc8454157caacec6241'   # evidence D
PR_REF = 'refs/heads/review/p3-c11-d2a1-final-source-proof-readiness-01'
PR_PACKET = 'docs/evidence/p3-candidate11/final-source-proof-readiness-01/'
TRANSPORT = '8693e3a742ee4d3917cbfdb809b377bcd919d9f4'   # AD1R2 packet: transported proof copies
DEST_REF = 'refs/heads/review/p3-c11-d2a1-final-baseline-known-runs-01'
DEST_LOCAL = 'refs/heads/evidence/p3-c11-d2a1-final-baseline-known-runs-01'
REMOTE = 'github'

CRATE = 'crates/nexus-governed-control/'
# path: (git blob, sha256 or None); the mission's blobs, and BUILD-INPUT-CLOSURE.md section 1 for the rest.
PINNED = {
    CRATE + 'src/ledger/protocol.rs': ('a2df2841b99cad5d452008a6d4150e9369d07e66',
                                       'a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296'),
    CRATE + 'src/ledger/protocol/model_tests.rs': ('28085ce232827301104b5d757af042ec933976f8', None),
    CRATE + 'src/ledger/protocol/unit_tests.rs': ('55dc66409eaccdaa93e8f0a56f2ccb14be81e114',
                                                  '841220b1095f339568e2764c41be0792a83c7fb097df6cdea8b50e46dc261159'),
    CRATE + 'src/ledger.rs': ('d872b90816fdd18cca8e5d1a0005bb49434aaf2a',
                              'd37983f6b040f67fd012f1bc4d040523a254d5d0557a39edfa40dd9dffe99215'),
    CRATE + 'src/lib.rs': ('b806ed577f8d3472b7187d161246e08d2f1ccd3d',
                           'addad1993f57ad370da7e38719f3e58d26b5a4eb1fbe06b24df51525b135727d'),
    'Cargo.lock': ('1af53a60aaf6fde5731e63fcd5b6cd11d86f669a',
                   '6ed84a9aeba4a542b1820c53d4ba0f4899cdfa819380aaa8649c3b5d31add979'),
    'rust-toolchain.toml': ('726005b29561a04536edea412bf551a9884d6639',
                            'fc7f9811196c9d9db1864dd140fa4b7632cda9b7761ed8736256b463c835d298'),
    'Cargo.toml': ('2c558a4477d1545daaa489f0a0e502b4284ad479',
                   '7f09c3dbe16a37bba215eeaa5f028614f3f6eafdf3419a5b79472f6b7cebf795'),
    CRATE + 'Cargo.toml': ('2543c745d661203326367962714b94c811e07980',
                           '30b2bb495434e85fc5b1e1476949711d55e8bb55cb9e0e6783bbc71ce0a3564f'),
    '.cargo/config.toml': ('05a42b50174e235b183b220afeb58922c96d17a1',
                           '379a150667985722540bd30f737cfb363dc5bc2dc8a5edae3db3725e3cd0b234'),
    'kernel/build.rs': (None, '5dd672bf7812edfc0fe41be7f6002f973ae87adec0eebfb92ae1f17d33e88da2'),
    'docs/evidence/p3-candidate11/current-tree-183/manifest.json': (
        '5d31cb45123403626f06c521dd2d53103676250e',
        '20bcc6fe221238f81246348cbbe94636a3c9b6d8acf89901d230e091f1a44279'),
}
AD1R4_PATH = 'docs/evidence/p3-candidate11/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md'
AD1R4_SHA = '00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693'
SEALED = '/home/nexus/nexus-audits/claude-architect/'
SEALED_FILES = {
    'p3-c11-dr01-d2a1-ad1r4-consolidation-01/drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md': AD1R4_SHA,
    'p3-c11-dr01-d2a1-adm-quotient-01/receipts/stage2/01-formal-argument.txt':
        'c75e7311fdc0eba53b366d5d8fcd11064c2de14d2a7c7bbaf8aace2816b9dd23',
    'p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt':
        '2e6feb4fd4baf6853665fb812441c6daf169858dc531dcb53ef0126303b0bc6a',
    'p3-c11-dr01-d2a1-closure-prep-01/receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt':
        'c2618c19a972826e1b310c289a0075c9954aa6ae6722ac38cf11976846fbb98f',
    'p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage5/36-negatives-and-fail-closed.txt':
        '2c517f358002e4e99449b554ec8242952640f177269ae89e9430984a4257f1e6',
}
TRANSPORTED = {  # transported copy at TRANSPORT: git blob of each sealed proof document
    'docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-adm-quotient-01/receipts/stage2/'
    '01-formal-argument.txt': '591b75a5ce574bbc35222b0adcad25d0f886336e',
    'docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-gc-symmetry-01/receipts/stage2/'
    '00-FORMAL-SYMMETRY-ANALYSIS.txt': 'd9e5260723cb7e28438fbb7a886dca32ac30dc77',
    'docs/evidence/p3-candidate11/ad1r2-review-packet-01/evidence/p3-c11-dr01-d2a1-closure-prep-01/receipts/stage2/'
    '00-FPT-FB-SYMMETRY-PROOF.txt': '8f8bcd7cdddb9b4177a7151915ab4538b386d746',
}

failures = []


def git(repo, *args, ok=(0,)):
    done = subprocess.run(['git', '-C', repo, *args], capture_output=True)
    if done.returncode not in ok:
        raise SystemExit('git %s failed (%d): %s' % (' '.join(args), done.returncode, done.stderr.decode()))
    return done.stdout


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def check(label, got, want):
    good = got == want
    print('%-6s %s: %s%s' % ('OK' if good else 'FAIL', label, got, '' if good else '  (expected %s)' % want))
    if not good:
        failures.append(label)


def commit(repo, c):
    raw = git(repo, 'cat-file', 'commit', c).decode()
    head = raw.partition('\n\n')[0].splitlines()
    return ([l[5:] for l in head if l.startswith('tree ')][0], [l[7:] for l in head if l.startswith('parent ')])


def tree_map(repo, c):
    out = {}
    for e in git(repo, 'ls-tree', '-r', '-z', c).split(b'\0'):
        if e:
            meta, path = e.split(b'\t', 1)
            out[path.decode()] = tuple(meta.decode().split())
    return out


def main(repo):
    print('AUTHENTICATION — P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01 (read-only; repository %s)' % repo)
    for label, c, tree, parents in (('C', C, C_TREE, [C_PARENT]), ('proof-readiness evidence', PR, PR_TREE, [PR_PARENT])):
        check('%s object type' % label, git(repo, 'cat-file', '-t', c).decode().strip(), 'commit')
        t, p = commit(repo, c)
        check('%s tree' % label, t, tree)
        check('%s parents (single)' % label, p, parents)
    check('C parent object type', git(repo, 'cat-file', '-t', C_PARENT).decode().strip(), 'commit')

    for path, (blob, digest) in PINNED.items():
        if blob:
            check('C:%s blob' % path, git(repo, 'rev-parse', '%s:%s' % (C, path)).decode().strip(), blob)
        if digest:
            check('C:%s sha256' % path, sha256(git(repo, 'cat-file', 'blob', '%s:%s' % (C, path))), digest)

    for c in (C_PARENT, C, PR):
        check('%s:AD-1-R4 sha256' % c[:8], sha256(git(repo, 'cat-file', 'blob', '%s:%s' % (c, AD1R4_PATH))), AD1R4_SHA)

    for rel, digest in SEALED_FILES.items():
        path = os.path.join(SEALED, rel)
        check('sealed %s sha256' % rel, sha256(open(path, 'rb').read()) if os.path.exists(path) else 'ABSENT', digest)
    for rel, blob in TRANSPORTED.items():
        check('%s:%s blob' % (TRANSPORT[:8], rel.rsplit('/', 3)[-3] + '/…/' + rel.rsplit('/', 1)[-1]),
              git(repo, 'rev-parse', '%s:%s' % (TRANSPORT, rel)).decode().strip(), blob)

    # The proof-readiness packet verifies against its own SHA256SUMS, read from git objects.
    sums = git(repo, 'cat-file', 'blob', '%s:%sSHA256SUMS' % (PR, PR_PACKET)).decode().splitlines()
    bad = []
    for line in sums:
        digest, _, name = line.partition('  ')
        name = name.lstrip('./')
        data = git(repo, 'cat-file', 'blob', '%s:%s%s' % (PR, PR_PACKET, name))
        if sha256(data) != digest:
            bad.append(name)
    check('proof-readiness packet: %d SHA256SUMS entries verified, mismatches' % len(sums), bad, [])

    # Outside docs/evidence/, the proof-readiness commit's tree equals C's: the evidence commit's parent carries
    # Candidate C's exact sources.
    tc, tp = tree_map(repo, C), tree_map(repo, PR)
    diff = sorted(p for p in set(tc) | set(tp) if tc.get(p) != tp.get(p) and not p.startswith('docs/evidence/'))
    check('paths outside docs/evidence/ differing between C and the proof-readiness commit', diff, [])
    check('C vs its parent: changed paths', sorted(p for p in set(tc) | set(tree_map(repo, C_PARENT))
                                                    if tc.get(p) != tree_map(repo, C_PARENT).get(p)),
          ['docs/evidence/p3-candidate11/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md',
           'docs/evidence/p3-candidate11/current-tree-183/manifest.json'])

    # Refs.
    remote = dict(reversed(l.split('\t')) for l in git(repo, 'ls-remote', REMOTE, PR_REF, DEST_REF).decode().splitlines())
    check('remote %s' % PR_REF, remote.get(PR_REF), PR)
    check('remote %s (must be absent)' % DEST_REF, remote.get(DEST_REF), None)
    local = git(repo, 'for-each-ref', '--format=%(refname) %(objectname)').decode().splitlines()
    local = dict(l.split(' ') for l in local)
    check('local refs/heads/evidence/p3-c11-d2a1-final-source-proof-readiness-01',
          local.get('refs/heads/evidence/p3-c11-d2a1-final-source-proof-readiness-01'), PR)
    check('local refs/remotes/%s/review/p3-c11-d2a1-final-source-proof-readiness-01' % REMOTE,
          local.get('refs/remotes/%s/review/p3-c11-d2a1-final-source-proof-readiness-01' % REMOTE), PR)
    check('local %s (must be absent)' % DEST_LOCAL, local.get(DEST_LOCAL), None)
    check('local refs/remotes/%s/review/p3-c11-d2a1-final-baseline-known-runs-01 (must be absent)' % REMOTE,
          local.get('refs/remotes/%s/review/p3-c11-d2a1-final-baseline-known-runs-01' % REMOTE), None)

    print('RESULT: %s' % ('ALL CHECKS HOLD' if not failures else 'MISMATCH: ' + '; '.join(failures)))
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
