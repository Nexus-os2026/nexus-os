"""PR-2 inert host verifier; never authenticates."""
import os
def host_context(): raise NotImplementedError('PR-2 host context')
def host_ancestry(*args): raise NotImplementedError('PR-2 protected ancestry')
