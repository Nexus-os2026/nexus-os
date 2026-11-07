"""Submission output is informational; only the host verifier authenticates."""
import argparse
import json
from pathlib import Path
import socket
import struct
import time

def exact(sock, count):
    data = bytearray()
    while len(data) < count:
        part = sock.recv(count-len(data))
        if not part: raise ValueError('truncated supervisor reply')
        data.extend(part)
    return bytes(data)

def submit(request):
    data = json.dumps(request, separators=(',',':')).encode()
    if not 1 <= len(data) <= 1024: raise ValueError('request bound')
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as sock:
        sock.settimeout(2)
        sock.connect('/run/mothergate-submit.sock')
        sock.sendall(struct.pack('!I',len(data))+data)
        sock.shutdown(socket.SHUT_WR)
        length = int.from_bytes(exact(sock,4),'big')
        if not 1 <= length <= 4096: raise ValueError('reply bound')
        return json.loads(exact(sock,length))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('repo','sha','gateset'): parser.add_argument('--'+key,required=True)
    parser.add_argument('--wait',action='store_true')
    args = parser.parse_args()
    reply = submit(dict(repo_id=args.repo,full_sha=args.sha,gate_set_id=args.gateset))
    print(json.dumps(reply),flush=True)
    if reply.get('status') != 'ACCEPTED': raise SystemExit(1)
    run = reply.get('run_id','')
    if len(run) != 32 or any(c not in '0123456789abcdef' for c in run): raise ValueError('reply ID')
    if args.wait:
        deadline = time.monotonic()+14500
        path = Path('/var/lib/mothergate/receipts')/run/'summary.json'
        while time.monotonic() < deadline:
            if path.is_file():
                summary = json.loads(path.read_bytes())
                print(json.dumps({'authentication':'UNAUTHENTICATED','summary':summary}))
                raise SystemExit(0 if summary.get('status') == 'PASS_SCOPED' else 1)
            time.sleep(.5)
        raise SystemExit('receipt wait deadline; accepted attempt remains recorded')

if __name__ == '__main__': main()
