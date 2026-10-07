from pathlib import Path
import subprocess,json,ast,hashlib,re
R=Path('/tmp/nexus-development-harness-v1'); M=R/'.nexus/missions/NEXUS-HARNESS-V1'
known=['AGENTS.md','CLAUDE.md','.gitignore','scripts/agent_harness.py','scripts/verify-agent-harness.py','scripts/tests/test_agent_harness.py']
names=['mission','implement','verify','review','mutation','handover','recover']
allowed_prefix=['.nexus/harness/','.nexus/missions/NEXUS-HARNESS-V1/']+[f'{p}/skills/nexus-{n}/' for p in ['.agents','.claude'] for n in names]
changed=subprocess.check_output(['git','diff','--name-only'],cwd=R,text=True).splitlines()
new=subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=R,text=True).splitlines()
for n in changed+new:
 assert n in known or n=='.nexus/missions/README.md' or any(n.startswith(p) for p in allowed_prefix),n
 p=R/n;assert not p.is_symlink(),n
 if p.suffix=='.py':ast.parse(p.read_text())
 # Scan only the scoped new source/docs, not credential locations or prior mission logs.
 if p.suffix in ['.py','.md','.json','.jsonl','.txt']:
  t=p.read_text()
  assert not re.search(r'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|\b(?:gh[pousr]_[A-Za-z0-9]{30,}|sk-[A-Za-z0-9]{40,})\b',t),n
ignored=['.nexus/runtime-private.json','.claude/skills/unapproved-skill/SKILL.md']
for n in ignored:assert subprocess.run(['git','check-ignore','-q','--no-index',n],cwd=R).returncode==0,n
for n in ['.nexus/harness/POLICY.md','.nexus/missions/NEXUS-HARNESS-V1/MISSION.md']+[f'.claude/skills/nexus-{x}/SKILL.md' for x in names]:assert subprocess.run(['git','check-ignore','-q','--no-index',n],cwd=R).returncode==1,n
print(json.dumps({'scope':'VALID','changed_existing':changed,'created_files':len(new),'python_syntax':'VALID','symlinks':'NONE','targeted_secret_pattern_scan':'NO_MATCHES (not exhaustive secret proof)','ignore_exceptions':'NARROW'},indent=2))
