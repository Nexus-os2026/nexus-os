/**
 * swarmBus — centralized event bus for the `swarm:event` Tauri channel.
 *
 * Single subscription, multi-consumer fan-out with a 200-entry ring buffer
 * so components that mount mid-stream can replay the recent past. Every
 * component that needs swarm events subscribes through this bus; no page
 * calls `listen("swarm:event", ...)` directly.
 *
 * Design notes:
 *   - `start()` must be idempotent. React.StrictMode double-invokes mount
 *     effects in dev, and multiple consumer pages may also call `start()`
 *     defensively. The second call returns early with a one-line warning.
 *   - The Tauri listen() handle is a Promise<UnlistenFn>. We store the
 *     promise so `stop()` can await it and release the native listener.
 *   - Subscriber exceptions are swallowed after being logged to the
 *     console — one buggy consumer must not break the other consumers or
 *     the bus itself.
 */

import type { SwarmEvent } from "./types";

const CHANNEL = "swarm:event";
const RING_CAPACITY = 200;

type Subscriber = (ev: SwarmEvent) => void;
type UnlistenFn = () => void;

class SwarmBus {
  private started = false;
  private unlistenPromise: Promise<UnlistenFn> | null = null;
  private readonly subscribers = new Set<Subscriber>();
  private readonly ring: SwarmEvent[] = [];

  /** Idempotent. Safe to call multiple times; second+ calls warn and no-op. */
  async start(): Promise<void> {
    if (this.started) {
      // eslint-disable-next-line no-console
      console.warn("[swarmBus] start() called while already started — ignoring");
      return;
    }
    this.started = true;

    try {
      const mod = await import("@tauri-apps/api/event");
      this.unlistenPromise = mod.listen<SwarmEvent>(CHANNEL, (ev) => {
        this.dispatch(ev.payload);
      });
      // Surface listener construction failures; don't leave `started` true.
      await this.unlistenPromise;
    } catch (err) {
      this.started = false;
      this.unlistenPromise = null;
      // eslint-disable-next-line no-console
      console.error("[swarmBus] failed to attach listener:", err);
      throw err;
    }
  }

  /** Subscribe for events. Returns an unsubscribe function. */
  subscribe(fn: Subscriber): () => void {
    this.subscribers.add(fn);
    return () => {
      this.subscribers.delete(fn);
    };
  }

  /** Return up to `lastN` most-recent events in chronological order. */
  replay(lastN: number): SwarmEvent[] {
    if (lastN <= 0) return [];
    if (lastN >= this.ring.length) return [...this.ring];
    return this.ring.slice(this.ring.length - lastN);
  }

  /** Release the native listener and reset state. */
  async stop(): Promise<void> {
    if (!this.started) return;
    this.started = false;
    const pending = this.unlistenPromise;
    this.unlistenPromise = null;
    if (pending) {
      try {
        const unlisten = await pending;
        unlisten();
      } catch (err) {
        // eslint-disable-next-line no-console
        console.error("[swarmBus] stop() unlisten failed:", err);
      }
    }
  }

  /**
   * Test-only entry point used by `swarmBus.__injectForTest` — see
   * `__injectForTest`. Also invoked internally by the Tauri listener.
   */
  private dispatch(ev: SwarmEvent): void {
    this.ring.push(ev);
    if (this.ring.length > RING_CAPACITY) {
      this.ring.splice(0, this.ring.length - RING_CAPACITY);
    }
    // Snapshot to tolerate subscribers that unsubscribe mid-iteration.
    const snapshot = [...this.subscribers];
    for (const fn of snapshot) {
      try {
        fn(ev);
      } catch (err) {
        // eslint-disable-next-line no-console
        console.error("[swarmBus] subscriber threw:", err);
      }
    }
  }

  /**
   * Test-only: inject an event as if it came off the Tauri channel. Lets
   * unit tests exercise the ring buffer, fan-out, and isolation semantics
   * without booting a real Tauri listener. Not exported in the public
   * type because consumers should never call this.
   */
  __injectForTest(ev: SwarmEvent): void {
    this.dispatch(ev);
  }

  /** Test-only accessor for current state assertions. */
  __stateForTest(): { started: boolean; subscriberCount: number; ringSize: number } {
    return {
      started: this.started,
      subscriberCount: this.subscribers.size,
      ringSize: this.ring.length,
    };
  }

  /**
   * Test-only hard reset. Clears ring buffer, subscribers, and listener
   * state. Do not use in production — callers should `stop()` + rebuild
   * instead, which preserves subscribers by design (page navigation should
   * not unsubscribe the bus's downstream store).
   */
  async __resetForTest(): Promise<void> {
    await this.stop();
    this.subscribers.clear();
    this.ring.length = 0;
  }
}

export const swarmBus = new SwarmBus();
export type { SwarmEvent } from "./types";
