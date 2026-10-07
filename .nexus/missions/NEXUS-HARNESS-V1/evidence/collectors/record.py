from pathlib import Path
import subprocess, json, datetime, hashlib, platform, os, sys
R=Path('/tmp/nexus-development-harness-v1'); M=R/'.nexus/missions/NEXUS-HARNESS-V1'; E=M/'evidence'
def sha(b): return hashlib.sha256(b).hexdigest()
def git(*args): return subprocess.check_output(['git',*args],cwd=R,env={**os.environ,'GIT_OPTIONAL_LOCKS':'0'},text=True).strip()
def append(r):
    with (E/'receipts.jsonl').open('a') as f: f.write(json.dumps(r,sort_keys=True)+'\n')
def snapshot():
    paths=[R/'AGENTS.md',R/'CLAUDE.md',R/'.gitignore']
    for d in ['.nexus/harness','.agents/skills','.claude/skills','scripts']:
        for p in (R/d).rglob('*'):
            if p.is_file() and '__pycache__' not in p.parts: paths.append(p)
    entries={str(p.relative_to(R)):sha(p.read_bytes()) for p in sorted(set(paths))}
    return sha(json.dumps(entries,sort_keys=True,separators=(',',':')).encode())
if sys.argv[1]=='bootstrap':
    for i,c in enumerate(json.loads((E/'preflight-commands.json').read_text())):
        append({'receipt_id':f'preflight-{i}','mission_id':'NEXUS-HARNESS-V1','timestamp_start':c['timestamp_start'],'timestamp_end':c['timestamp_end'],'cwd':c['cwd'],'candidate_sha':'9d5fdf95920bdeddb7453911f86390417b6a4604','candidate_tree':'6eb07f71d8a704290e950aac6b092f801dc51cf2','command':c['command'],'command_hash':sha(json.dumps(c['command'],separators=(',',':')).encode()),'exit_code':c['exit_code'],'status':'EXITED','stdout_ref':c['stdout'],'stderr_ref':c['stderr'],'raw_log_hash':{k:sha((E/c[k+'_ref'] if k+'_ref' in c else E/c[k]).read_bytes()) for k in ['stdout','stderr']},'environment':{'platform':platform.platform(),'python':platform.python_version()},'acceptance_criterion':'A: establish baseline and isolation','control_ids':[],'notes':'Executed from C9 checkout read-only. GitHub main output identifies the separate harness base, not C9.'})
else:
    rid=sys.argv[1]; args=sys.argv[2:]; start=datetime.datetime.now(datetime.timezone.utc).isoformat(); head=git('rev-parse','HEAD'); tree=git('rev-parse','HEAD^{tree}'); snap=snapshot()
    p=subprocess.run(args,cwd=R,capture_output=True,env={**os.environ,'GIT_OPTIONAL_LOCKS':'0','PYTHONDONTWRITEBYTECODE':'1'})
    refs={}
    for k in ['stdout','stderr']:
        refs[k]=f'logs/{rid}.{k}.txt'; (E/refs[k]).write_bytes(getattr(p,k))
    append({'receipt_id':rid,'mission_id':'NEXUS-HARNESS-V1','timestamp_start':start,'timestamp_end':datetime.datetime.now(datetime.timezone.utc).isoformat(),'cwd':str(R),'candidate_sha':head,'candidate_tree':tree,'working_snapshot_sha256':snap,'command':args,'command_hash':sha(json.dumps(args,separators=(',',':')).encode()),'exit_code':p.returncode,'status':'EXITED','stdout_ref':refs['stdout'],'stderr_ref':refs['stderr'],'raw_log_hash':{k:sha(getattr(p,k)) for k in refs},'environment':{'platform':platform.platform(),'python':platform.python_version()},'acceptance_criterion':rid,'control_ids':[],'notes':'HEAD/tree measured before execution; uncommitted harness bytes bound separately by working_snapshot_sha256 (AGENTS, CLAUDE, .gitignore, harness, provider skills, scripts; mission records excluded). No independent acceptance.'})
    print(p.stdout.decode(errors='replace')); print(p.stderr.decode(errors='replace')); print(f'{rid}: exit {p.returncode}'); sys.exit(p.returncode)
