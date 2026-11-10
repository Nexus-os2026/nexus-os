#!/usr/bin/python3 -IB
"""Only protected host storage authenticates. Detached copies never do.
SB3E-R19.4/R21.6: the submitter runs this installed verifier through one exact sudoers rule,
`verify.py --authenticate-stdin`, which takes no other argument and reads exactly one 32-hex run ID
from stdin; `identity` says whether the receipt was produced by the current installed identity.
Credit needs AUTHENTICATED and current, never a copied summary. -B: root writes no bytecode (R21.5)."""
import argparse
import json
import os
from pathlib import Path
import pwd
import re
import stat
import sys
import importlib.util
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('gbx_receipt',HERE/'receipt.py')
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)
ROOT = Path('/var/lib/mothergate/receipts')
IDENTITY = Path('/etc/gatebox/identity.json')

def installed_identity():
    for item in [*reversed(IDENTITY.parents),IDENTITY]:
        info = item.lstat()
        if info.st_uid != 0 or info.st_mode & 0o022 or stat.S_ISLNK(info.st_mode):
            raise ValueError('UNAUTHENTICATED: unprotected installed identity')
    return json.loads(IDENTITY.read_bytes(),object_pairs_hook=receipt.unique)

def host_context():
    if os.geteuid() != 0 or Path('/proc/self/uid_map').read_text().split() != ['0','0','4294967295']:
        raise ValueError('UNAUTHENTICATED: host root ownership namespace required')

def host_ancestry(path):
    path = Path(path)
    if path != ROOT: raise ValueError('UNAUTHENTICATED: detached receipt storage')
    owner = pwd.getpwnam('mothergate').pw_uid
    for ancestor in [*reversed(path.parents),path]:
        info = ancestor.lstat()
        expected = owner if ancestor == path else 0
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != expected or info.st_mode & 0o022:
            raise ValueError('UNAUTHENTICATED: unsafe protected ancestry')
        fd = receipt.open_directory(ancestor)
        try: receipt.no_acl(fd)
        finally: os.close(fd)

def authenticate(run_id):
    host_context()
    host_ancestry(ROOT)
    if not receipt.RUN.fullmatch(run_id): raise ValueError('invalid receipt ID')
    result = receipt.verify_integrity(ROOT/run_id,pwd.getpwnam('mothergate').pw_uid)
    result['authentication'] = 'AUTHENTICATED'
    installed = result['summary']['installed']
    current = installed_identity()
    result['identity'] = 'current' if {k:installed.get(k) for k in current} == current else 'historical'
    return result

def stdin_run_id(argv, stream):
    """The fixed sudo mode: no other argument, one bounded lowercase 32-hex ID and at most one LF."""
    if argv != ['--authenticate-stdin']: raise SystemExit('--authenticate-stdin takes no other argument')
    data = stream.read(34)
    if not re.fullmatch(rb'[0-9a-f]{32}\n?',data): raise ValueError('invalid receipt ID')
    return data[:32].decode()

def main():
    if '--authenticate-stdin' in sys.argv[1:]:
        print(json.dumps(authenticate(stdin_run_id(sys.argv[1:],sys.stdin.buffer)),sort_keys=True))
        return
    parser = argparse.ArgumentParser()
    select = parser.add_mutually_exclusive_group(required=True)
    select.add_argument('--run-id')
    select.add_argument('--latest',action='store_true')
    for name in ('repo','sha','gateset','inventory-sha256','require-status'): parser.add_argument('--'+name)
    args = parser.parse_args()
    host_context()
    host_ancestry(ROOT)
    if args.latest:
        if not all((args.repo,args.sha,args.gateset)): parser.error('latest needs repo, sha and gateset')
        expected = receipt.request_for(args.repo,args.sha,args.gateset,args.inventory_sha256)
        matches = []
        for path in ROOT.iterdir():
            item = authenticate(path.name)
            req = item['summary']['request']
            if req == expected:
                matches.append((path.stat().st_mtime_ns,path.name,item))
        if not matches: raise ValueError('no matching authenticated receipt')
        result = max(matches)[2]
    else: result = authenticate(args.run_id)
    print(json.dumps(result,sort_keys=True))
    if args.require_status and result['summary']['status'] != args.require_status:
        raise SystemExit('authentic receipt does not have required gate status')
if __name__ == '__main__': main()
