import { describe, expect, it } from "vitest";
import {
  AUDIT_EVENT_KINDS,
  auditKindCategory,
} from "../audit_kind_category";
import type { AuditEventKind } from "../types";

describe("auditKindCategory", () => {
  it("maps every AuditEventKind to a valid EventCategory", () => {
    // Exhaustiveness sanity: the union has 7 members; the mapper
    // must handle each. The TypeScript `never` guard inside the
    // function would catch a missing case at compile time; this
    // test is the runtime mirror.
    expect(AUDIT_EVENT_KINDS).toHaveLength(7);
    for (const kind of AUDIT_EVENT_KINDS) {
      const cat = auditKindCategory(kind);
      expect(["plan", "node", "oracle", "provider", "swarm"]).toContain(cat);
    }
  });

  it("groups node_* kinds under 'node'", () => {
    const nodeKinds: readonly AuditEventKind[] = [
      "node_started",
      "node_event",
      "node_completed",
      "node_failed",
    ];
    for (const k of nodeKinds) {
      expect(auditKindCategory(k)).toBe("node");
    }
  });

  it("groups oracle_* kinds under 'oracle'", () => {
    expect(auditKindCategory("oracle_runtime_check")).toBe("oracle");
    expect(auditKindCategory("oracle_runtime_denial")).toBe("oracle");
  });

  it("maps budget_update to 'swarm'", () => {
    expect(auditKindCategory("budget_update")).toBe("swarm");
  });
});
