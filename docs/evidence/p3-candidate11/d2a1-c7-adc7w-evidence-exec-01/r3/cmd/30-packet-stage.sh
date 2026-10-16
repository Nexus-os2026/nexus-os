# Publication 1: stage the review packet (EXEC-01 section 13) in a NEW directory; scan it; write its SHA256SUMS. Nothing touches git here.
. /home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c7-w-evidence-exec-01/r3/cmd/00-gates.sh 30-packet-stage 8589934592
set -e
K=$EV/publication/packet
[ ! -e $K ] || { echo "REFUSED: $K exists"; exit 93; }
mkdir -m 700 $K
cp -p $EV/publication/README.md $K/README.md
cp -rp $EV/receipts $K/receipts
mkdir $K/r3
cp -p $EV/r3/MISSION-R3.txt $EV/r3/01-PRESERVATION.txt $EV/r3/90-LOCAL-CLASSIFICATION.txt $EV/r3/PROGRESS.md $K/r3/
cp -rp $EV/r3/cmd $EV/r3/gates $EV/r3/console $K/r3/
F=$EV/preflight
cd $F
for f in \
  opus-r2-35e734c1/preflight-r2/COMMANDS.txt opus-r2-35e734c1/preflight-r2/RESULTS.txt opus-r2-35e734c1/preflight-r2/MANIFEST.sha256 \
  opus-r2-35e734c1/preflight-r2/TOOLS.sha256 opus-r2-35e734c1/preflight-r2/FIXTURES.sha256 \
  opus-r2-35e734c1/preflight-r2/gate-run3.txt opus-r2-35e734c1/preflight-r2/gate-run3.stdout opus-r2-35e734c1/preflight-r2/gate-run3.exit \
  opus-r2-35e734c1/preflight-r2/gate-run3.disk opus-r2-35e734c1/preflight-r2/gate-run3.hashcheck \
  opus-r2-35e734c1/preflight-r2/st-B-sup-enforcement/SELFTEST.txt opus-r2-35e734c1/ran-gate-run3.txt \
  $(for s in st-A-sup-build-invoke st-B-sup-enforcement st-wt st-bwc st-class st-link; do for x in exit disk hashcheck stdout; do echo opus-r2-35e734c1/preflight-r2/$s.$x; done; done) \
  opus-r1-f4afe1e2/PROGRESS.md opus-r1-f4afe1e2/expected.txt opus-r1-f4afe1e2/FROZEN-TOOLS.sha256.txt \
  sonnet-7b42496f/gateout/gate-run1.txt sonnet-7b42496f/gateout/gate-run2.txt sonnet-7b42496f/gateout/gate-run2.stdout sonnet-7b42496f/mission.txt \
  $(for t in c7bwc.py c7class.py c7class_selftest.py c7lib.py c7link.py c7sup.py c7wt.py gate.py selftest_sup.py selftest_wt.py scope-wrap.sh; do echo sonnet-7b42496f/c7tools/$t; done) \
  sonnet-7b42496f/samples/C4-healthy-raw.txt sonnet-7b42496f/samples/C4-mutant-raw.txt sonnet-7b42496f/samples/C9-fullfpt-raw.txt \
  $(for n in 1 2 3 4 5 6 7; do echo sonnet-7b42496f/listfix/f$n.txt; done) \
  sonnet-7b42496f/listcrate/Cargo.toml sonnet-7b42496f/listcrate/Cargo.lock sonnet-7b42496f/listcrate/rust-toolchain.toml sonnet-7b42496f/listcrate/src/lib.rs \
  opus-r1-f4afe1e2.SOURCE.manifest opus-r1-f4afe1e2.COPY.manifest opus-r2-35e734c1.SOURCE.manifest opus-r2-35e734c1.COPY.manifest \
  sonnet-7b42496f.SOURCE.manifest sonnet-7b42496f.COPY.manifest; do
  mkdir -p $K/preflight/$(dirname $f); cp -p $f $K/preflight/$f; cmp $f $K/preflight/$f
done
cd $K
chmod -R u+w .
echo "== scans"
echo "symlinks: $(find . -type l | wc -l); non-regular: $(find . ! -type f ! -type d | wc -l); files: $(find . -type f | wc -l); bytes: $(du -sb . | cut -f1)"
echo "files over 1 MiB: $(find . -type f -size +1024k | wc -l)"
echo "ELF files: $(find . -type f -exec sh -c 'head -c4 "$1" | grep -q ELF && echo "$1"' _ {} \; | wc -l)"
echo "files named protocol.rs: $(find . -name protocol.rs | wc -l)"
/usr/bin/python3 - <<'EOF'
import os, hashlib, re
bad, cred = [], []
pat = re.compile(rb"(gh[opsu]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----|AKIA[0-9A-Z]{16})")
for r, _, fs in os.walk("."):
    for f in fs:
        b = open(os.path.join(r, f), "rb").read()
        if hashlib.sha1(b"blob %d\0" % len(b) + b).hexdigest() == "0d4ab9929c0e15d910735a52b72a5f3855bef58a" or (b.count(b"\n") > 1000 and b"self.core.refusals.clear();" in b):
            bad.append(os.path.join(r, f))
        if pat.search(b):
            cred.append(os.path.join(r, f))
print("files that are or contain the mutated protocol source:", bad)
print("files matching credential patterns:", cred)
raise SystemExit(1 if bad or cred else 0)
EOF
[ $(find . -type l | wc -l) -eq 0 ] && [ $(find . -type f -size +1024k | wc -l) -eq 0 ] && [ $(find . -name protocol.rs | wc -l) -eq 0 ]
find . -type f ! -name SHA256SUMS -printf '%P\n' | LC_ALL=C sort | while IFS= read -r p; do sha256sum "$p"; done > SHA256SUMS
echo "SHA256SUMS: $(wc -l < SHA256SUMS) entries; sha256 $(sha256sum < SHA256SUMS | cut -d' ' -f1)"
sha256sum --strict -c SHA256SUMS | awk '{print $NF}' | sort | uniq -c
echo "STAGE RESULT: OK"
