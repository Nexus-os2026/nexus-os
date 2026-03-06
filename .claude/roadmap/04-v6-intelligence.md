# Phase 4: v6.0+ - Agent Intelligence

## 4.1 Multi-Agent Collaboration (agents/collaboration/)
GovernedChannel, Orchestrator, Blackboard.
- [ ] Governed channels with rate limiting
- [ ] Orchestrator assigns tasks

## 4.2 Capability Delegation (kernel/src/delegation.rs)
DelegationEngine with transitive trust and cascade revocation.
- [ ] Delegation lifecycle works
- [ ] Kernel checks delegation engine

## 4.3 Adaptive Governance (kernel/src/adaptive_policy.rs)
Trust scores, promotion/demotion. Promotions require human approval.
- [ ] Trust score computation
- [ ] Auto-demotion works

## 4.4 Governed Fine-Tuning (research/src/fine_tuning.rs)
Safety checks on all training jobs. Human approval required.
- [ ] Job lifecycle with safety checks
- [ ] Full audit trail
