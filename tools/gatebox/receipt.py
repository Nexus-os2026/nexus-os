import os
"""PR-1 inert interfaces: contract checkpoint must be red."""
def validate_summary(summary):
    raise NotImplementedError("PR-1 inert interface")

def publish(pending, receipts, run_id, summary, artifacts):
    raise NotImplementedError("PR-1 inert interface")

def verify_integrity(root, owner_uid):
    raise NotImplementedError("PR-1 inert interface")

