#!/bin/bash
set -euo pipefail
exec /usr/bin/python3 -I -B - "$@" <<'PY'
FILES = ['install.sh','probe.py']
PLACEHOLDER = {'identity':None,'passed':[]}
def render(text,uid,gid): raise NotImplementedError('render')
def bundle_files(path,digest): raise NotImplementedError('verified bundle')
def acceptance_check(identity,value,state,authenticate): raise NotImplementedError('acceptance binding')
def main(): raise NotImplementedError('install')
if __name__ == '__main__': main()
PY
