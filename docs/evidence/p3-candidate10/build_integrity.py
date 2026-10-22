"""Phase Three harness-only source and Cargo build identity barrier.

SOURCE RESTORATION != EXECUTION RESTORATION.  A clean Git status proves source
bytes, not which bytes a warm Cargo executable was compiled from.  The caller
must serialize use of its explicit target directory while this guard runs.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
import tomllib


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class BuildIntegrity:
    def __init__(self, root, target, environment, evidence):
        self.root = Path(root).resolve()
        self.target = Path(target).resolve()
        self.environment = dict(environment)
        self.evidence = Path(evidence).resolve()
        self.evidence.mkdir(parents=True, exist_ok=True)
        assert self.environment.get('CARGO_TARGET_DIR') == str(self.target)
        assert self.target.is_dir() and not self.evidence.is_relative_to(self.root)
        self.pending = {}
        self.committed = {}
        self.sequence = 0

    def record(self, event):
        event = dict(event, time_utc_ns=time.time_ns())
        with (self.evidence/'events.jsonl').open('a') as output:
            output.write(json.dumps(event, sort_keys=True)+'\n')
        return event

    def package_for(self, relative):
        if not str(relative).endswith('.rs'):
            return None
        path = (self.root/relative).resolve()
        assert path.is_relative_to(self.root) and path.is_file() and not path.is_symlink()
        for directory in path.parents:
            manifest = directory/'Cargo.toml'
            if manifest.is_file():
                package = tomllib.loads(manifest.read_text()).get('package')
                if package:
                    return package['name']
            if directory == self.root:
                break
        raise AssertionError(f'Rust source lacks package: {relative}')

    def packages_for(self, relatives):
        return {package for name in relatives if (package := self.package_for(name))}

    def committed_bytes(self, relative):
        if relative not in self.committed:
            assert not Path(relative).is_absolute() and '..' not in Path(relative).parts
            self.committed[relative] = subprocess.check_output(
                ['git', '--no-optional-locks', '-C', str(self.root), 'show', 'HEAD:'+relative])
        return self.committed[relative]

    def verify_source(self, relative, expected):
        path = self.root/relative
        actual = path.read_bytes()
        assert actual == expected, (relative, 'restored bytes')
        assert sha256(actual) == sha256(expected), (relative, 'SHA-256')
        assert actual == self.committed_bytes(relative), (relative, 'Git blob')
        return {'path':relative, 'sha256':sha256(actual)}

    def artifact_mtime_ns(self, packages):
        newest = 0
        deps = self.target/'debug/deps'
        fingerprints = self.target/'debug/.fingerprint'
        incremental = self.target/'debug/incremental'
        for package in sorted(packages):
            stem = package.replace('-', '_')
            candidates = []
            if deps.is_dir():
                candidates.extend(deps.glob(stem+'-*'))
                candidates.extend(deps.glob('lib'+stem+'-*'))
            if fingerprints.is_dir():
                for directory in fingerprints.glob(package+'-*'):
                    candidates.append(directory)
                    candidates.extend(p for p in directory.rglob('*') if p.is_file())
            if incremental.is_dir():
                candidates.extend(incremental.glob(stem+'-*'))
            for path in candidates:
                try:
                    newest = max(newest, path.stat().st_mtime_ns)
                except FileNotFoundError:
                    continue
        return newest

    def barrier(self, relatives, packages, reason, timeout=3.0):
        paths = [self.root/name for name in sorted(set(relatives))]
        assert paths
        for path in paths:
            assert path.is_file() and not path.is_symlink() and path.resolve().is_relative_to(self.root)
        artifact_ns = self.artifact_mtime_ns(packages)
        deadline = time.monotonic()+timeout
        while time.monotonic() < deadline:
            now = time.time_ns()
            if now <= artifact_ns:
                time.sleep(0.01)
                continue
            for path in paths:
                stat = path.stat()
                os.utime(path, ns=(stat.st_atime_ns, now))
            observed = {str(path.relative_to(self.root)):path.stat().st_mtime_ns for path in paths}
            if all(value > artifact_ns and value <= time.time_ns() for value in observed.values()):
                return self.record({'kind':'mtime_barrier', 'reason':reason,
                    'packages':sorted(packages), 'artifact_mtime_ns':artifact_ns,
                    'source_mtime_ns':observed, 'strict':True})
            time.sleep(0.01)
        raise RuntimeError(f'could not establish non-future strict mtime barrier: {reason}')

    def refresh_tracked(self, packages, reason='pristine baseline entry'):
        names = subprocess.check_output(['git','--no-optional-locks','-C',str(self.root),'ls-files','-z']).split(b'\0')
        tracked = []
        for raw in names:
            if raw:
                name = os.fsdecode(raw)
                path = self.root/name
                if path.is_file() and not path.is_symlink():
                    tracked.append(name)
        result = self.barrier(tracked, packages, reason)
        assert subprocess.check_output(['git','--no-optional-locks','-C',str(self.root),
            'status','--porcelain=v1','--untracked-files=all']) == b''
        return result

    def restore(self, originals, packages, reason):
        hashes = []
        for relative, content in originals.items():
            (self.root/relative).write_bytes(content)
            hashes.append(self.verify_source(relative, content))
        assert subprocess.check_output(['git','--no-optional-locks','-C',str(self.root),
            'status','--porcelain=v1','--untracked-files=all']) == b'', 'restored worktree is not clean'
        barrier = self.barrier(originals, packages, reason)
        for package in packages:
            self.pending[package] = {'source_hashes':hashes, 'barrier':barrier}
        self.record({'kind':'source_restored', 'reason':reason, 'hashes':hashes,
                     'pending_packages':sorted(self.pending)})
        return hashes

    def _execute(self, command, cwd, label, timeout):
        self.sequence += 1
        safe = re.sub(r'[^A-Za-z0-9_.-]', '_', label)
        log = self.evidence/f'{self.sequence:04d}-{safe}.log'
        cargo_at = command.index('cargo')
        actual = command[:cargo_at+1]+['-vv']+command[cargo_at+1:]
        timed_out = False
        with log.open('xb') as output:
            process = subprocess.Popen(actual, cwd=cwd, env=self.environment,
                stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                process.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
                timed_out = True
        return {'command':actual, 'returncode':process.returncode, 'timeout':timed_out,
                'log':str(log), 'text':log.read_text(errors='replace')}

    @staticmethod
    def compiled(text, package):
        return bool(re.search(r'^\s*Compiling '+re.escape(package)+r' v\S+', text, re.M))

    def clean_package(self, package, reason):
        command=['cargo','clean','-p',package,'--target-dir',str(self.target)]
        proc=subprocess.run(command,cwd=self.root,env=self.environment,
                            stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=120)
        self.record({'kind':'package_clean_fallback','package':package,'reason':reason,
                     'command':command,'returncode':proc.returncode,'output':proc.stdout})
        if proc.returncode:
            raise RuntimeError(f'package-scoped clean failed for {package}')

    def run_cargo(self, command, cwd, label, timeout, require_rebuild=()):
        required=set(require_rebuild)
        assert not self.pending or required.issuperset(self.pending), (
            'pending mutant package lacks rebuild proof', sorted(self.pending))
        assert 'cargo' in command and self.environment['CARGO_TARGET_DIR']==str(self.target)
        first=self._execute(command,cwd,label+'-first',timeout)
        missing={package for package in required if not self.compiled(first['text'],package)}
        if missing:
            for package in sorted(missing):
                self.clean_package(package,label+' lacked Compiling proof')
            final=self._execute(command,cwd,label+'-after-clean',timeout)
            assert all(self.compiled(final['text'],package) for package in missing), (
                label,'rebuild not proven after package clean',sorted(missing))
        else:
            final=first
        proof=self.record({'kind':'cargo_build_identity','label':label,'required_packages':sorted(required),
            'first_log':first['log'],'first_returncode':first['returncode'],
            'first_fresh_packages':sorted(missing),'final_log':final['log'],
            'final_returncode':final['returncode'],'final_timeout':final['timeout'],
            'compiled_packages':sorted(p for p in required if self.compiled(final['text'],p)),
            'clean_fallback':bool(missing)})
        return final, proof

    def prove_pending(self, reason):
        for package in sorted(list(self.pending)):
            command=['cargo','test','-p',package,'--locked','--lib','--no-run']
            result, proof=self.run_cargo(command,self.root,reason+'-'+package,900,{package})
            if result['returncode'] or result['timeout']:
                raise RuntimeError(f'pristine package rebuild failed: {package}')
            self.record({'kind':'pending_cleared','package':package,'reason':reason,
                         'source_hashes':self.pending[package]['source_hashes'],
                         'build_proof':proof})
            del self.pending[package]
        assert not self.pending
