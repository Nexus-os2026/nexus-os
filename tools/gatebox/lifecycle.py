#!/usr/bin/python3 -I
"""PR-2 inert privileged shim; no command is executed."""
def command(*args): raise NotImplementedError('PR-2 shim')
def check_start(*args): raise NotImplementedError('PR-2 start validation')
