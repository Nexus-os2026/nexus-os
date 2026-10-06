#!/usr/bin/env python3
"""Adapted Candidate-10 canonical campaign; never the original 183 harness.

check: read-only anchor/provenance validation (works before committing).
baseline: execute all distinct intended commands, without applying mutations.
campaign: one complete M01-M183 run, after exact-SHA gate and baseline receipts.
review: validate a separate per-control semantic review against immutable logs.

An intended test failure is an observation, not yet a proved kill. The review
step requires a quoted failure and a property explanation for every such result.
Compiler failures, zero tests, timeouts and infrastructure failures never count.
No subset or resume mode exists: partial campaigns cannot be promoted or merged.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
MANIFEST = HERE / 'manifest.json'
BASE = '9d5fdf95920bdeddb7453911f86390417b6a4604'
REQUIRED_GATES = {
    'fmt', 'clippy', 'workspace-tests', 'npm-gate', 'npm-ci', 'tsc',
    'frontend-tests', 'vite-build', 'builder-assembly', 'builder-entry',
    'builder-packaged', 'package-inspection', 'webview-dev', 'webview-release',
    'live-confirmation', 'live-browser', 'rust-audit', 'c9-baseline', 'new-tests',
}

def digest(data):
    return hashlib.sha256(data).hexdigest()

def read_json(path):
    return json.loads(Path(path).read_text())

def write_json(path, value):
    # Output directories are exclusive to one run; preserve old evidence.
    with Path(path).open('x') as out:
        json.dump(value, out, indent=2, ensure_ascii=False)
        out.write('\n')

def git(root, *args):
    return subprocess.check_output(['git', '--no-optional-locks', '-C', str(root), *args]).decode().strip()

def status(root):
    return subprocess.check_output(['git','--no-optional-locks','-C',str(root),'status','--porcelain=v1','--untracked-files=all']).decode()

def identity(root):
    return dict(sha=git(root,'rev-parse','HEAD'),tree=git(root,'rev-parse','HEAD^{tree}'),
                runner_sha256=digest(Path(__file__).read_bytes()), manifest_sha256=digest(MANIFEST.read_bytes()))

def source(root, relative):
    candidate=root / relative
    assert not Path(relative).is_absolute() and '..' not in Path(relative).parts, relative
    assert candidate.is_file() and not candidate.is_symlink(), relative
    assert candidate.resolve().is_relative_to(root), relative
    return candidate

def edited(root, control):
    before={}
    after={}
    for edit in control['edits']:
        name=edit['file']; path=source(root,name)
        if name not in before:
            before[name]=path.read_bytes(); after[name]=before[name]
        anchor=edit['anchor']; replacement=edit['replacement'].encode()
        if anchor is None:
            after[name]+=replacement
        else:
            anchor=anchor.encode()
            assert after[name].count(anchor)==edit['count'], (control['id'],name,'anchor count')
            after[name]=after[name].replace(anchor,replacement,edit['count'])
    assert any(after[name]!=before[name] for name in before), control['id']
    return before,after

def check(root, manifest):
    assert manifest['name']=='adapted Candidate-10 canonical campaign'
    controls=manifest['controls']
    assert [c['id'] for c in controls]==[f'M{i:02}' for i in range(1,184)]
    assert manifest['historical_corrected_harness']['sha256']=='9bdd045121e10fa03d35b300283b0bbc487b3ef29d69ad7e00d33a6a8c607623'
    assert controls[165]['lineage']=='VERSION-SUPERSEDED'
    assert len(controls[159]['edits'])==2
    for control in controls:
        before,_=edited(root,control)
        assert {name:digest(data) for name,data in before.items()}==control['source_sha256'], (control['id'],'source changed since semantic review')
        assert control['expected_failure_tests'], control['id']
    return controls

def run(root, control, env, log, timeout):
    start=time.time()
    timed_out=False
    with log.open('xb') as output:
        proc=subprocess.Popen(control['command'], cwd=root/control.get('cwd','.'), env=env,
                              stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            proc.wait(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt):
            # Signal only the process group this runner created and still owns.
            os.killpg(proc.pid,signal.SIGKILL)
            proc.wait()
            timed_out=True
    text=re.sub(r'\x1b\[[0-9;]*m','',log.read_text(errors='replace'))
    rust='cargo' in control['command']
    failed=re.findall(r'^test (.+?) \.\.\. FAILED\s*$',text,re.M)
    passed=re.findall(r'^test (.+?) \.\.\. ok\s*$',text,re.M)
    if rust:
        reached=bool(re.search(r'Running (?:unittests|tests/)',text)) and bool(failed or passed)
        intended_failed=sorted(set(failed)&set(control['expected_failure_tests']))
        intended_passed=set(control['expected_failure_tests'])<=set(passed)
        compilation_failed=bool(re.search(r'error\[E\d+\]|error: could not compile|error: linking with',text))
    else:
        reached='Test Files' in text and 'Tests' in text
        intended_failed=[name for name in control['expected_failure_tests'] if re.search(r'FAIL\s+[^\n]*'+re.escape(name),text)]
        intended_passed=all(name in text for name in control['expected_failure_tests']) and proc.returncode==0
        compilation_failed=False
    if timed_out:
        observation='INFRA ERROR'
    elif compilation_failed:
        observation='COMPILE ERROR'
    elif proc.returncode==0 and reached and intended_passed:
        observation='SURVIVED'
    elif proc.returncode!=0 and reached and intended_failed:
        observation='INTENDED TEST FAILURE'
    else:
        observation='INFRA ERROR'
    return dict(command=control['command'],cwd=control.get('cwd','.'),returncode=proc.returncode,
                duration_seconds=round(time.time()-start,3),timeout=timed_out,compiled_to_test=reached,
                failed_tests=failed,intended_failed_tests=intended_failed,
                intended_baseline_passed=intended_passed,observation=observation,
                log=log.name,log_sha256=digest(log.read_bytes()))

def receipt(path, expected, required):
    data=read_json(path)
    assert data['identity']==expected, 'receipt belongs to another source/harness SHA'
    assert data['status']=='PASS', 'baseline gates have not passed'
    jobs=data['jobs']
    assert required<=set(jobs), ('missing baseline gates',required-set(jobs))
    for name,job in jobs.items():
        assert job['returncode']==0, name
        log=Path(path).parent/job['log']
        assert digest(log.read_bytes())==job['log_sha256'], (name,'log hash')
    return data

def refresh_tracked(root):
    # A warm cache may originate in another worktree. Refresh input mtimes,
    # never content or Git refs, before this checkout's baseline/campaign.
    for name in subprocess.check_output(['git','-C',str(root),'ls-files','-z']).split(b'\0'):
        if name:
            path=root/os.fsdecode(name)
            if path.is_file() and not path.is_symlink(): os.utime(path,None)

def review(directory, review_path):
    campaign=read_json(directory/'campaign.json')
    rows=read_json(directory/'observations.json')
    decisions=read_json(review_path)
    assert digest((directory/'observations.json').read_bytes())==campaign['observations_sha256']
    assert campaign['complete'] and campaign['restored'] and len(rows)==183
    assert [row['id'] for row in rows]==[f'M{i:02}' for i in range(1,184)]
    assert decisions['campaign_sha256']==digest((directory/'campaign.json').read_bytes())
    reviews=decisions['controls']; assert set(reviews)=={row['id'] for row in rows}
    result=[]
    for row in rows:
        note=reviews[row['id']]
        assert note['log_sha256']==row['log_sha256']==digest((directory/row['log']).read_bytes())
        assert len(note['explanation'])>=40
        if row['observation']=='INTENDED TEST FAILURE':
            assert note['verdict'] in ['KILLED','INFRA ERROR']
            text=(directory/row['log']).read_text(errors='replace')
            assert len(note['quote'])>=12 and note['quote'] in text, row['id']
            if note['verdict']=='KILLED': assert row['compiled_to_test'] and row['restored']
        else:
            assert note['verdict']==row['observation'], row['id']
        result.append(dict(id=row['id'],**note))
    totals=dict(collections.Counter(row['verdict'] for row in result))
    final=dict(identity=campaign['identity'],totals=totals,controls=result,
               campaign_sha256=decisions['campaign_sha256'],review_sha256=digest(Path(review_path).read_bytes()),
               status='PASS' if totals=={'KILLED':183} else 'FAIL')
    write_json(directory/'reviewed-results.json',final)
    print(json.dumps(totals),flush=True)
    return 0 if final['status']=='PASS' else 1

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode',choices=['check','baseline','campaign','review'])
    parser.add_argument('--root',type=Path)
    parser.add_argument('--sha')
    parser.add_argument('--out',type=Path)
    parser.add_argument('--environment',type=Path)
    parser.add_argument('--gates',type=Path)
    parser.add_argument('--baseline',type=Path)
    parser.add_argument('--decisions',type=Path)
    parser.add_argument('--timeout',type=int,default=900)
    args=parser.parse_args()
    if args.mode=='review': return review(args.out.resolve(),args.decisions)
    root=args.root.resolve(); manifest=read_json(MANIFEST); controls=check(root,manifest)
    if args.mode=='check':
        print('183 controls: ordered anchors, source hashes, lineage and expected tests validated; no mutation applied')
        return 0
    assert args.sha and re.fullmatch('[0-9a-f]{40}',args.sha)
    ident=identity(root)
    assert ident['sha']==args.sha and ident['sha']!=BASE
    assert git(root,'branch','--show-current')=='repair/p3-candidate10-closure'
    subprocess.run(['git','-C',str(root),'merge-base','--is-ancestor',BASE,args.sha],check=True)
    original_status=status(root); assert original_status=='', 'checkout must be clean'
    env=read_json(args.environment)
    assert env.get('CI')=='1' and not env.get('DISPLAY') and not env.get('DBUS_SESSION_BUS_ADDRESS')
    assert Path(env['HOME']).is_dir() and Path(env['CARGO_TARGET_DIR']).is_dir()
    out=args.out.resolve(); assert not out.is_relative_to(root), 'evidence belongs outside the checkout'
    if args.mode=='campaign':
        assert args.gates and args.baseline
        receipt(args.gates,ident,REQUIRED_GATES)
        baseline=receipt(args.baseline,ident,set())
        assert baseline['control_ids']==[c['id'] for c in controls]
        assert baseline['all_intended_tests_passed'] is True
    out.mkdir(parents=True,exist_ok=False)
    write_json(out/'identity.json',dict(**ident,historical=manifest['historical_candidate'],
        historical_corrected_harness=manifest['historical_corrected_harness'],
        m166=controls[165],environment=env,git_status_before=original_status))
    refresh_tracked(root)
    if args.mode=='baseline':
        jobs={}; by_command={}; all_pass=True
        for control in controls:
            key=json.dumps([control['cwd'],control['command']])
            if key in by_command: continue
            name=control['id']; by_command[key]=name
            # Commands shared by controls must cover every intended judge.
            merged=dict(control)
            merged['expected_failure_tests']=sorted({n for c in controls if json.dumps([c['cwd'],c['command']])==key for n in c['expected_failure_tests']})
            result=run(root,merged,env,out/(name+'.log'),args.timeout)
            passed=result['returncode']==0 and result['intended_baseline_passed'] and result['compiled_to_test']
            jobs[name]=result
            all_pass &= passed
            print(name,'BASELINE PASS' if passed else 'BASELINE FAIL',flush=True)
            if not passed: break
        assert status(root)==original_status and identity(root)==ident, 'baseline changed source state'
        write_json(out/'baseline.json',dict(identity=ident,jobs=jobs,control_ids=[c['id'] for c in controls],
                    all_intended_tests_passed=all_pass,status='PASS' if all_pass else 'FAIL'))
        return 0 if all_pass else 1
    results=[]; restored=True
    all_files={edit['file'] for c in controls for edit in c['edits']}
    before_hashes={name:digest(source(root,name).read_bytes()) for name in sorted(all_files)}
    write_json(out/'source-before.json',before_hashes)
    backups=out/'backups';backups.mkdir()
    for name in sorted(all_files):
        path=backups/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(source(root,name).read_bytes())
    try:
        for control in controls:
            assert identity(root)==ident and status(root)==original_status
            before,after=edited(root,control)
            result=dict(id=control['id'],observation='INFRA ERROR',compiled_to_test=False,
                        before_sha256={name:digest(data) for name,data in before.items()})
            try:
                for name,data in after.items(): source(root,name).write_bytes(data)
                result['mutant_sha256']={name:digest(source(root,name).read_bytes()) for name in after}
                result.update(run(root,control,env,out/(control['id']+'.log'),args.timeout))
            finally:
                for name,data in before.items(): source(root,name).write_bytes(data)
                result['after_sha256']={name:digest(source(root,name).read_bytes()) for name in before}
                result['git_status_after']=status(root)
                result['restored']=result['before_sha256']==result['after_sha256'] and result['git_status_after']==original_status
                restored &= result['restored']
                results.append(result)
                write_json(out/(control['id']+'.json'),result)
            print(control['id'],result['observation'], 'RESTORED' if restored else 'RESTORATION FAILED',flush=True)
            if not restored: break
    finally:
        after_hashes={name:digest(source(root,name).read_bytes()) for name in sorted(all_files)}
        write_json(out/'source-after.json',after_hashes)
        restored &= after_hashes==before_hashes and status(root)==original_status and identity(root)==ident
        write_json(out/'observations.json',results)
        write_json(out/'campaign.json',dict(identity=ident,complete=len(results)==183,restored=restored,
            controls_executed=len(results),git_status_after=status(root),
            historical_corrected_harness=manifest['historical_corrected_harness'],
            m166_lineage='VERSION-SUPERSEDED',observations_sha256=digest((out/'observations.json').read_bytes()),
            semantic_review='REQUIRED; intended test failures are not yet proved kills'))
    return 0 if len(results)==183 and restored and all(r['observation']=='INTENDED TEST FAILURE' for r in results) else 1

if __name__=='__main__':
    raise SystemExit(main())
