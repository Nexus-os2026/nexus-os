"""Submission output is informational; only the host verifier authenticates."""
import argparse
import importlib.util
import json
from pathlib import Path
import socket
import struct
import time
spec = importlib.util.spec_from_file_location('gbx_receipt',Path(__file__).resolve().with_name('receipt.py'))
receipt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(receipt)

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

def read_summary(path):
    """Informational only: an unauthenticated copy that the host verifier must still authenticate."""
    return json.loads(path.read_bytes()) if path.is_file() else None

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for key in ('repo','sha','gateset'): parser.add_argument('--'+key,required=True)
    parser.add_argument('--inventory-sha256',help='route-cells: the reviewed harness name-list commitment')
    parser.add_argument('--route-pins-sha256',help="route-cells: sha256 of the commit's tools/route_pins.json (GB3)")
    parser.add_argument('--wait',action='store_true')
    args = parser.parse_args()
    request = receipt.request_for(args.repo,args.sha,args.gateset,args.inventory_sha256,args.route_pins_sha256)
    success = receipt.GATE_SETS[args.gateset]['status']
    reply = submit(request)
    print(json.dumps(reply),flush=True)
    if reply.get('status') != 'ACCEPTED': raise SystemExit(1)
    run = reply.get('run_id','')
    if len(run) != 32 or any(c not in '0123456789abcdef' for c in run): raise ValueError('reply ID')
    if args.wait:
        deadline = time.monotonic()+14500
        path = Path('/var/lib/mothergate/receipts')/run/'summary.json'
        while time.monotonic() < deadline:
            summary = read_summary(path)
            if summary is not None:
                print(json.dumps({'authentication':'UNAUTHENTICATED','summary':summary}))
                raise SystemExit(0 if summary.get('status') == success else 1)
            time.sleep(.5)
        raise SystemExit('receipt wait deadline; accepted attempt remains recorded')

if __name__ == '__main__': main()
