# Security Audit Skill for Nexus OS

When asked to run a security audit, check for:

## Rust Backend
- SQL injection in any raw queries
- Unsafe blocks that aren't documented
- Secrets/API keys hardcoded in source
- Missing input validation on Tauri commands
- Unhandled errors that could leak info

## TypeScript Frontend
- XSS vulnerabilities
- Exposed API keys in frontend code
- Missing CSRF protection
- Insecure direct object references

## Agents
- Prompt injection vulnerabilities
- Capability escalation paths
- Data exfiltration risks
- Governance bypass attempts

## Run these checks:
```bash
cargo audit 2>&1 | tail -20
grep -rn "api_key\|password\|secret\|token" src/ --include="*.ts" | grep -v ".env" | grep -v "test"
grep -rn "innerHTML\|dangerouslySetInnerHTML" app/src/ --include="*.tsx"
grep -rn "eval(\|exec(" app/src/ --include="*.ts"
```
