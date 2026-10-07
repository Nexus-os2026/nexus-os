from pathlib import Path
import hashlib,json,datetime
E=Path('/tmp/nexus-development-harness-v1/.nexus/missions/NEXUS-HARNESS-V1/evidence')
artifacts={str(p.relative_to(E)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'size':p.stat().st_size} for p in sorted(E.rglob('*')) if p.is_file() and p.name!='manifest.json'}
x={'schema_version':1,'mission_id':'NEXUS-HARNESS-V1','sealed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'artifacts':artifacts,'limitations':'Agent-produced integrity index, not independent acceptance; manifest excludes itself. Final clean-commit execution is in the external post-commit seal named in HANDOVER.'}
(E/'manifest.json').write_text(json.dumps(x,indent=2)+'\n')
print(f'Sealed {len(artifacts)} evidence artifacts')
