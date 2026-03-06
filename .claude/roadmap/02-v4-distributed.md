# Phase 2: v4.0 - Distributed Governance

## 2.1 Cross-Node Replication (distributed/)
New crate: node.rs, replication.rs, transport.rs, membership.rs.
- [ ] Audit events replicate across nodes
- [ ] Failure detection works
- [ ] 3-node integration test

## 2.2 Quorum Execution (distributed/src/quorum.rs)
QuorumEngine: propose/vote/timeout. Wire into kernel autonomy gate.
- [ ] Quorum lifecycle works
- [ ] Kernel consults quorum for high-risk actions

## 2.3 Federated Audit (kernel/src/audit/federation.rs)
Cross-node hash references for tamper-evident federation.
- [ ] Federation verification command
- [ ] Proofs exportable

## 2.4 Marketplace (marketplace/)
Registry, Ed25519 manifest verification, install flow.
- [ ] Signature verification blocks tampered agents
- [ ] CLI for marketplace
