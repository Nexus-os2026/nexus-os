# NEXUS OS Threat Model

## Scope

This document captures known risks for NEXUS OS v1.0.0 and the mitigations currently implemented.

## Security Principles

- Don’t trust. Verify.
- Explicit capabilities and least privilege
- Auditability and tamper evidence
- Human approval for authority-changing actions

## Assets

- Agent manifests and capability grants
- Runtime state and fuel budgets
- Connector credentials and tokens
- Update metadata and release artifacts
- Audit logs and telemetry data

## Trust Boundaries

- Local runtime vs external APIs/connectors
- Signed update metadata vs downloaded package payloads
- User-approved actions vs autonomous behavior
- Production update path vs research-preview patching

## Threats and Mitigations

### 1. Malicious or tampered updates
- Threat: package/signature tampering, provenance spoofing
- Mitigations:
  - Ed25519 package signatures
  - in-toto provenance validation
  - TUF verification (root/targets/snapshot/timestamp)
  - rollback and freeze attack protection
  - canary rollout with rollback

### 2. Capability escalation
- Threat: unauthorized actions beyond declared permissions
- Mitigations:
  - capability checks in runtime gateways
  - manifest capability validation
  - adaptation authority policy (`never allowed` classes)
  - self-patch DSL blocks capability edits

### 3. Audit bypass / repudiation
- Threat: hiding or altering event traces
- Mitigations:
  - hash-chained audit trail integrity
  - policy blocks for audit bypass mutations
  - mutation lifecycle attestation events

### 4. Remote command abuse
- Threat: unauthorized mobile/messaging actions
- Mitigations:
  - step-up authentication and cryptographic challenges
  - stronger auth requirement for remote creation/deploy
  - explicit approval flow before deployment

### 5. Prompt injection and unsafe LLM outputs
- Threat: instruction hijack, unsafe tool invocation
- Mitigations:
  - governed LLM gateway capability and fuel checks
  - LLM defense validation and circuit breaker
  - output action validation and audit logging

### 6. Data leakage in telemetry/reports
- Threat: sensitive identifiers leaked to telemetry
- Mitigations:
  - telemetry opt-in default OFF
  - anonymized report payloads (hashed identifiers)
  - report redaction patterns for API keys/tokens

## Residual Risks

- Third-party API schema changes can break connector semantics before updates are available.
- Human approval quality varies and remains a social/operational risk.
- LLM model behavior drift may impact generated strategy quality.

## Future Hardening

- Sigstore/cosign verification chain for release artifacts
- Hardware-backed key storage for signing roles
- Continuous chaos testing for rollback pathways
- Mandatory SLSA provenance levels for all release builds
