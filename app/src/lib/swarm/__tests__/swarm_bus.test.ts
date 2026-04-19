import { describe, it, expect, vi, beforeEach } from "vitest";
import { swarmBus } from "../swarm_bus";
import type { SwarmEvent } from "../types";

function fakeProviderEvent(id: string): SwarmEvent {
  return {
    event: "provider_health_update",
    providers: [
      {
        provider_id: id,
        status: "Ok",
        latency_ms: 12,
        models: [],
        notes: "",
        checked_at_secs: 0,
      },
    ],
  };
}

beforeEach(async () => {
  // Hard reset bus state between tests: release the native listener and
  // wipe the ring + subscribers so ordering assertions are deterministic.
  await swarmBus.__resetForTest();
});

describe("swarmBus", () => {
  it("start → subscribe → inject → subscriber receives", async () => {
    await swarmBus.start();
    const seen: SwarmEvent[] = [];
    const unsub = swarmBus.subscribe((ev) => {
      seen.push(ev);
    });
    swarmBus.__injectForTest(fakeProviderEvent("ollama"));
    expect(seen).toHaveLength(1);
    expect(seen[0].event).toBe("provider_health_update");
    unsub();
  });

  it("fans out to multiple subscribers", async () => {
    await swarmBus.start();
    const a: SwarmEvent[] = [];
    const b: SwarmEvent[] = [];
    const c: SwarmEvent[] = [];
    swarmBus.subscribe((ev) => a.push(ev));
    swarmBus.subscribe((ev) => b.push(ev));
    swarmBus.subscribe((ev) => c.push(ev));
    swarmBus.__injectForTest(fakeProviderEvent("anthropic"));
    expect(a).toHaveLength(1);
    expect(b).toHaveLength(1);
    expect(c).toHaveLength(1);
  });

  it("unsubscribe removes exactly one subscriber", async () => {
    await swarmBus.start();
    const kept: SwarmEvent[] = [];
    const dropped: SwarmEvent[] = [];
    swarmBus.subscribe((ev) => kept.push(ev));
    const off = swarmBus.subscribe((ev) => dropped.push(ev));
    off();
    swarmBus.__injectForTest(fakeProviderEvent("openrouter"));
    expect(kept).toHaveLength(1);
    expect(dropped).toHaveLength(0);
  });

  it("ring buffer returns the most recent N events", async () => {
    await swarmBus.start();
    for (let i = 0; i < 250; i += 1) {
      swarmBus.__injectForTest(fakeProviderEvent(`p${i}`));
    }
    const last50 = swarmBus.replay(50);
    expect(last50).toHaveLength(50);
    const first = last50[0];
    const last = last50[last50.length - 1];
    if (first.event !== "provider_health_update" || last.event !== "provider_health_update") {
      throw new Error("ring buffer returned unexpected event shape");
    }
    // The ring's capacity is 200; after 250 events the oldest retained
    // entry is index 50 (p50), and replay(50) returns entries 200–249.
    expect(first.providers[0].provider_id).toBe("p200");
    expect(last.providers[0].provider_id).toBe("p249");
  });

  it("double-start is idempotent and logs a warning", async () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    await swarmBus.start();
    await swarmBus.start();
    expect(warn).toHaveBeenCalledTimes(1);
    const msg = String(warn.mock.calls[0][0]);
    expect(msg).toContain("already started");
    warn.mockRestore();
  });

  it("a throwing subscriber does not prevent other subscribers from firing", async () => {
    await swarmBus.start();
    const err = vi.spyOn(console, "error").mockImplementation(() => {});
    const healthy: SwarmEvent[] = [];
    swarmBus.subscribe(() => {
      throw new Error("boom");
    });
    swarmBus.subscribe((ev) => healthy.push(ev));
    swarmBus.__injectForTest(fakeProviderEvent("ollama"));
    expect(healthy).toHaveLength(1);
    expect(err).toHaveBeenCalled();
    err.mockRestore();
  });

  it("stop() resets started so start() can be called again", async () => {
    await swarmBus.start();
    expect(swarmBus.__stateForTest().started).toBe(true);
    await swarmBus.stop();
    expect(swarmBus.__stateForTest().started).toBe(false);
    await swarmBus.start();
    expect(swarmBus.__stateForTest().started).toBe(true);
  });
});
