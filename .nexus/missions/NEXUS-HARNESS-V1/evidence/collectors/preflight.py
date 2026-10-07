import subprocess, json, hashlib, pathlib, datetime, os
root=pathlib.Path('/home/nexus/NEXUS/nexus-os-p3-candidate9-local')
out=pathlib.Path('/tmp/nexus-harness-v1-bootstrap')
def git(*args,cwd=root):
    return subprocess.check_output(['git',*args],cwd=cwd,env={**os.environ,'GIT_OPTIONAL_LOCKS':'0'},text=True)
def sha(b): return hashlib.sha256(b).hexdigest()
def tree_hash(path):
    entries={}
    for p in sorted(path.rglob('*')):
        if p.is_symlink(): entries[str(p.relative_to(path))]={'symlink':os.readlink(p)}
        elif p.is_file(): entries[str(p.relative_to(path))]={'sha256':sha(p.read_bytes()),'size':p.stat().st_size}
    return {'files':len(entries),'sha256':sha(json.dumps(entries,sort_keys=True,separators=(',',':')).encode()),'entries':entries}
start=datetime.datetime.now(datetime.timezone.utc).isoformat()
commands=[['status','-sb'],['rev-parse','HEAD'],['rev-parse','HEAD^{tree}'],['branch','--show-current'],['worktree','list','--porcelain'],['remote','-v'],['ls-remote','github','refs/heads/main']]
cmds=[]
for n,args in enumerate(commands):
    a=datetime.datetime.now(datetime.timezone.utc).isoformat(); p=subprocess.run(['git',*args],cwd=root,env={**os.environ,'GIT_OPTIONAL_LOCKS':'0'},capture_output=True)
    for stream in ['stdout','stderr']: (out/f'preflight-{n}.{stream}.txt').write_bytes(getattr(p,stream))
    cmds.append({'command':['git',*args],'cwd':str(root),'timestamp_start':a,'timestamp_end':datetime.datetime.now(datetime.timezone.utc).isoformat(),'exit_code':p.returncode,'stdout':f'preflight-{n}.stdout.txt','stderr':f'preflight-{n}.stderr.txt'})
    if p.returncode: raise SystemExit(f'preflight failed: {args}')
refs={line.split()[0]:line.split()[1] for line in git('for-each-ref','--format=%(refname) %(objectname)').splitlines() if not line.startswith('refs/codex/')}
worktrees=[]
for block in git('worktree','list','--porcelain').strip().split('\n\n'):
    path=block.splitlines()[0][9:]
    if 'candidate10' in path or 'c10-' in path:
        worktrees.append({'path':path,'head':git('rev-parse','HEAD',cwd=path).strip(),'tree':git('rev-parse','HEAD^{tree}',cwd=path).strip(),'status':git('status','--porcelain=v1','--untracked-files=all',cwd=path),'branch':git('branch','--show-current',cwd=path).strip()})
evidence=pathlib.Path('/home/nexus/NEXUS/phase3-candidate10-evidence')
snapshot={'timestamp':start,'refs':refs,'candidate10_worktrees':worktrees,'candidate10_evidence':{'path':str(evidence),**tree_hash(evidence)}}
(out/'isolation-before.json').write_text(json.dumps(snapshot,indent=2)+'\n')
(out/'preflight-commands.json').write_text(json.dumps(cmds,indent=2)+'\n')
base='6e15dee613ea35823d1adbf6d8917ed59eeda583'
for name in ['AGENTS.md','CLAUDE.md','tasks/todo.md','tasks/lessons.md']:
    (out/name.replace('/','-')).write_text(git('show',f'{base}:{name}'))
files=git('ls-tree','-r','--name-only',base).splitlines()
interesting=[f for f in files if f.startswith(('.claude/','.agents/','.codex/','tasks/','scripts/','.github/','docs/evidence/','docs/audits/'))]
(out/'inventory.txt').write_text('\n'.join(interesting)+'\n')
print(json.dumps({'protected_refs':len(refs),'candidate10_worktrees':worktrees,'candidate10_evidence_files':snapshot['candidate10_evidence']['files'],'base':base,'tree':git('rev-parse',base+'^{tree}').strip(),'original_agents_bytes':(out/'AGENTS.md').stat().st_size},indent=2))
