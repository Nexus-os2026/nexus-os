"""F10 verifies the whole primary chain before asserting native answer facts."""
import json
import sqlite3
import sys
from pathlib import Path
from evidence import inspect

path = Path(sys.argv[1]).resolve(strict=True)
inspect(path)
phase, reason = sys.argv[2:]
with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=3) as db:
    rows = db.execute("SELECT body FROM record WHERE phase=? ORDER BY seq", (phase,)).fetchall()
assert len(rows) == 1, (phase, "exactly one native dialogue result", rows)
facts = json.loads(rows[0][0])
if reason == "none":
    assert facts["failure"] is None, facts
    assert "reason" not in facts["detail"], facts
else:
    assert facts["failure"] == "unavailable", ("interruption is not an unqualified Owner decline", facts)
    assert facts["detail"].get("reason") == reason, ("exact typed answer survives privacy sealing", facts)
print("CONFIRMATION EVIDENCE VERIFIED", phase, reason, flush=True)
