"""Independent read-only verifier for the test Home's primary evidence.

Counts are derived only after checking the complete stored chain. This does
not read a production health/count projection or create a new store session.
"""
import hashlib
import json
import sqlite3
import sys
from pathlib import Path


def inspect(path):
    path = Path(path).resolve(strict=True)
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=3) as db:
        assert db.execute("PRAGMA application_id").fetchone() == (0x50334556,)
        assert db.execute("PRAGMA integrity_check").fetchall() == [("ok",)]
        identities = db.execute("SELECT one, store, format FROM identity").fetchall()
        assert len(identities) == 1
        one, store, version = identities[0]
        assert one == 1 and version == "p3-evidence/1"
        assert len(store) == 32 and all(c in "0123456789abcdef" for c in store)
        rows = db.execute(
            "SELECT seq,session,stored_ms,phase,commitment,body,prev,tag "
            "FROM record ORDER BY seq"
        ).fetchall()
        prior = "0" * 64
        phases = {}
        for expected, row in enumerate(rows, 1):
            seq, session, stored, phase, commitment, body, prev, tag = row
            assert seq == expected and prev == prior
            fields = [b"nexus.p3.evidence.tag.v1", store.encode(), prev.encode(),
                      seq.to_bytes(8, "big"), session.encode(),
                      stored.to_bytes(8, "big"), phase.encode()]
            fields += ([b"\x00"] if commitment is None else [b"\x01", commitment.encode()])
            fields += [body.encode()]
            encoded = b"".join(len(f).to_bytes(8, "big") + f for f in fields)
            assert hashlib.sha256(encoded).hexdigest() == tag
            facts = json.loads(body)
            if phase == "acknowledged" and commitment is None:
                assert facts.get("covers_through", 0) < seq
            phases[phase] = phases.get(phase, 0) + 1
            prior = tag
        return [phases.get(p, 0) for p in
                ("emergency_stop", "session_opened", "session_closed")]


if __name__ == "__main__":
    print("PRIMARY CHAIN VERIFIED " + " ".join(map(str, inspect(sys.argv[1]))), flush=True)
