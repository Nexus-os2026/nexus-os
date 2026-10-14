"""Shared constants and read-only git helpers.

Mission P3-C11-DR01-D2A1-FINAL-SOURCE-PROOF-READINESS-01. Nothing here writes to
the repository: every read is `git cat-file`, `git ls-tree`, `git rev-list` or
`git rev-parse` on immutable objects.
"""
import hashlib
import subprocess

# Candidate C, its parent, evidence D, and the proof/experimental lineage.
C = 'd5964ee5326c2733177307db6f507328023d361a'
C_TREE = '51bac5fd335999cc995c0ab9f582a30244973ef0'
C_PARENT = '71264aa81a40b86189783facc8437e0427887c16'
D = '066a322a9182fc978e3f5dc8454157caacec6241'
D_TREE = 'da31dfa7c9a0c4f358bfde4f15815f6c8d46f806'

SOURCE_R2 = '244c5b23244d1abd8b2b595c9e7654a25029a9f1'
ADM = '83816e66916c3a217ab7919d7e6a21893e7d579a'
GC = '77a146ca606f99329068410b6037721341f00c70'
EXT = '2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda'
W1 = '42dd1bd1cf21ffdea5968b256d2983f6575fab5b'
W4 = '04e42a7ddf0d0a2af17b6506c51997006e7a08dc'
B = 'a464a36621523f539d2554a6b176d678c7f885ea'

# Single-parent chain from SOURCE-R2 to D, in order (each is the next one's parent).
LINEAGE = [SOURCE_R2, ADM, GC, EXT, W1, W4, B, C_PARENT, C, D]
ROLE = {
    SOURCE_R2: 'SOURCE-R2 (model; ADM proof source read)',
    ADM: 'admission quotient (AQ) implementation; GC proof source read',
    GC: 'gating-caller symmetry (SQ) implementation; extension proof source read',
    EXT: 'F-PT/F-B extension implementation (SYMMETRIC allow-list)',
    W1: 'C1/C25 witness source',
    W4: 'C4 witness source (commit A)',
    B: 'C4 evidence commit B (documentation)',
    C_PARENT: 'AD-1-R4 review commit R4 (documentation)',
    C: 'Candidate C (source under review)',
    D: 'evidence D (transport only; parent of this packet)',
}
SHORT = {SOURCE_R2: 'SOURCE-R2', ADM: 'ADM', GC: 'GC', EXT: 'EXT', W1: 'W1', W4: 'W4', B: 'B',
         C_PARENT: 'R4', C: 'C', D: 'D'}
EXPECTED_TREE = {C: C_TREE, D: D_TREE, SOURCE_R2: '1e0c76d8ba6aa437d5d02367c81c4de9d46ffc78',
                 ADM: '74a6a0e5add063dac4d24939b3a62c619847dd3a', GC: '21818166ee10a8075ca364bd09f31c7aa64555dd',
                 EXT: '6f6a54f5c85d6a3102cd034c67e5791cb5a0286a', W1: '7ac6bb21430d15b17e3723b217b20a49dccf8282',
                 W4: 'fca5c2a28e1c6cfef3cd92fbb81484a56769a9b3'}

PROTO = 'crates/nexus-governed-control/src/ledger/protocol.rs'
MODEL = 'crates/nexus-governed-control/src/ledger/protocol/model_tests.rs'
UNIT = 'crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs'
LEDGER = 'crates/nexus-governed-control/src/ledger.rs'
LIB = 'crates/nexus-governed-control/src/lib.rs'
LOCK = 'Cargo.lock'
WS_TOML = 'Cargo.toml'
CRATE_TOML = 'crates/nexus-governed-control/Cargo.toml'
TOOLCHAIN = 'rust-toolchain.toml'
CARGO_CFG = '.cargo/config.toml'
FILES = [PROTO, MODEL, UNIT, LEDGER, LIB, LOCK, WS_TOML, CRATE_TOML, TOOLCHAIN, CARGO_CFG]
SOURCE_FILES = [PROTO, MODEL, UNIT, LEDGER, LIB]
BUILD_FILES = [LOCK, WS_TOML, CRATE_TOML, TOOLCHAIN, CARGO_CFG]

EVIDENCE_PREFIX = 'docs/evidence/'
PACKET = 'docs/evidence/p3-candidate11/final-source-proof-readiness-01/'
DEST_REF = 'refs/heads/review/p3-c11-d2a1-final-source-proof-readiness-01'


def git(repo, *args, ok=(0,), text=False):
    done = subprocess.run(['git', '-C', repo, *args], capture_output=True)
    if done.returncode not in ok:
        raise SystemExit('git %s failed (%d): %s' % (' '.join(args), done.returncode, done.stderr.decode()))
    return done.stdout.decode() if text else done.stdout


def blob(repo, commit, path):
    return git(repo, 'cat-file', 'blob', '%s:%s' % (commit, path))


def blob_id(repo, commit, path):
    return git(repo, 'rev-parse', '%s:%s' % (commit, path), text=True).strip()


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def git_blob_hash(data):
    return hashlib.sha1(b'blob %d\0' % len(data) + data).hexdigest()


def tree_map(repo, commit):
    """{path: (mode, type, object)} of every entry of the commit's tree, recursively."""
    out = {}
    for entry in git(repo, 'ls-tree', '-r', '-z', commit).split(b'\0'):
        if not entry:
            continue
        meta, path = entry.split(b'\t', 1)
        mode, kind, obj = meta.decode().split()
        out[path.decode()] = (mode, kind, obj)
    return out


def commit_info(repo, commit):
    raw = git(repo, 'cat-file', 'commit', commit, text=True)
    head, _, msg = raw.partition('\n\n')
    tree, parents = None, []
    for line in head.splitlines():
        if line.startswith('tree '):
            tree = line[5:]
        elif line.startswith('parent '):
            parents.append(line[7:])
    return {'commit': commit, 'tree': tree, 'parents': parents, 'subject': msg.splitlines()[0] if msg else ''}
