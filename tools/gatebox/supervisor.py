"""PR-2 inert supervisor interface; never launches a process."""
def receive_request(*args): raise NotImplementedError('PR-2 supervisor')
class Admission:
    def __init__(self,*args): raise NotImplementedError('PR-2 admission')
class Evidence:
    def __init__(self,*args,**kwargs): raise NotImplementedError('PR-2 evidence')
def qualified(*args): raise NotImplementedError('PR-2 qualification binding')

from pathlib import Path
from unittest.mock import Mock
snapshot, socket = Mock(), Mock()
STATE = Path('/var/lib/mothergate')
def sibling(*args): raise NotImplementedError('PR-2 sibling')
def shim(*args): raise NotImplementedError('PR-2 shim')
def collect(*args): raise NotImplementedError('PR-2 capture')
def coverage(*args): raise NotImplementedError('PR-2 coverage')
def stop_and_confirm(*args): raise NotImplementedError('PR-2 cleanup')
def execute(*args): raise NotImplementedError('PR-2 execution')
def admission_stage(*args): raise NotImplementedError('Q1 qualification admission')
