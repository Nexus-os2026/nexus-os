import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { mockCommands, expectInvokedWith } from "../../../test/setup";
import { DirectorConsole } from "../DirectorConsole";
import { swarmBus } from "../../../lib/swarm/swarm_bus";
import { __resetSwarmStoreForTest } from "../../../lib/swarm/store";
import type { PlannedSwarmJson, SwarmEvent } from "../../../lib/swarm/types";

function plannedResponse(): PlannedSwarmJson {
  return {
    dag: { nodes: [], edges: [] },
    ticket_id: "11111111-2222-3333-4444-555555555555",
    budget_hash: "deadbeef",
    privacy_envelope: "Public",
  };
}

function planProposedEvent(): SwarmEvent {
  return {
    event: "plan_proposed",
    run_id: "11111111-2222-3333-4444-555555555555",
    dag_json: { nodes: [], edges: [] },
  };
}

beforeEach(() => {
  __resetSwarmStoreForTest();
  mockCommands({
    swarm_plan: plannedResponse(),
    swarm_reject: null,
  });
});

describe("DirectorConsole", () => {
  it("renders textarea and submit button", () => {
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    expect(screen.getByTestId("director-textarea")).toBeInTheDocument();
    expect(screen.getByTestId("director-submit")).toBeInTheDocument();
  });

  it("submit calls swarm_plan with the entered intent and hands the response upward", async () => {
    const onPlanReady = vi.fn();
    render(
      <DirectorConsole
        onPlanReady={onPlanReady}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const ta = screen.getByTestId("director-textarea") as HTMLTextAreaElement;
    fireEvent.change(ta, { target: { value: "research top 3 papers" } });
    fireEvent.click(screen.getByTestId("director-submit"));
    await waitFor(() => {
      expectInvokedWith("swarm_plan", { intent: "research top 3 papers" });
    });
    expect(onPlanReady).toHaveBeenCalledTimes(1);
    expect(onPlanReady.mock.calls[0][0]).toMatchObject({
      ticket_id: "11111111-2222-3333-4444-555555555555",
      privacy_envelope: "Public",
    });
  });

  it("submit button is disabled while a plan is pending in the store", async () => {
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId="11111111-2222-3333-4444-555555555555"
        onPlanCleared={(): void => {}}
      />,
    );
    const ta = screen.getByTestId("director-textarea") as HTMLTextAreaElement;
    // Flip store into pending by dispatching plan_proposed through the bus.
    act(() => {
      swarmBus.__injectForTest(planProposedEvent());
    });
    fireEvent.change(ta, { target: { value: "anything" } });
    const submit = screen.getByTestId("director-submit") as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    expect(ta.disabled).toBe(true);
  });

  it("Escape dispatches swarm_reject with the pending ticket_id and clears parent state", async () => {
    const onPlanCleared = vi.fn();
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        onPlanCleared={onPlanCleared}
      />,
    );
    const ta = screen.getByTestId("director-textarea");
    fireEvent.keyDown(ta, { key: "Escape" });
    await waitFor(() => {
      expectInvokedWith("swarm_reject", { ticketId: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" });
    });
    await waitFor(() => expect(onPlanCleared).toHaveBeenCalledTimes(1));
  });
});

// ── Track C #3 commit 2: mic button + transcript append ──────────────────

import { appendTranscript } from "../DirectorConsole";
import { mockCommandError } from "../../../test/setup";

interface FakeAudioContext {
  sampleRate: number;
  destination: object;
  createMediaStreamSource: () => { connect: () => void; disconnect: () => void };
  createScriptProcessor: () => {
    connect: () => void;
    disconnect: () => void;
    onaudioprocess:
      | ((e: { inputBuffer: { getChannelData: () => Float32Array } }) => void)
      | null;
  };
  close: () => Promise<void>;
}

function installAudioStubs(): void {
  Object.defineProperty(window.navigator, "mediaDevices", {
    configurable: true,
    writable: true,
    value: {
      getUserMedia: () =>
        Promise.resolve({
          getTracks: () => [{ stop: () => {} }],
        } as unknown as MediaStream),
    },
  });
  const audioCtorImpl = function (this: FakeAudioContext): FakeAudioContext {
    this.sampleRate = 48_000;
    this.destination = {};
    this.createMediaStreamSource = () => ({
      connect: () => {},
      disconnect: () => {},
    });
    this.createScriptProcessor = () => ({
      connect: () => {},
      disconnect: () => {},
      onaudioprocess: null,
    });
    this.close = () => Promise.resolve();
    return this;
  } as unknown as { new (): FakeAudioContext };
  Object.defineProperty(window, "AudioContext", {
    configurable: true,
    writable: true,
    value: audioCtorImpl,
  });
}

describe("DirectorConsole mic button (Track C #3 commit 2)", () => {
  beforeEach(() => {
    __resetSwarmStoreForTest();
    installAudioStubs();
  });

  it("renders mic button disabled when pipeline_health unreachable", async () => {
    mockCommands({
      swarm_plan: plannedResponse(),
      swarm_reject: null,
      voice_pipeline_health: {
        reachable: false,
        model: null,
        last_error: "No module named 'faster_whisper'",
      },
    });
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const mic = screen.getByTestId("director-mic") as HTMLButtonElement;
    await waitFor(() => {
      expect(mic.getAttribute("title") ?? "").toContain("Voice unavailable:");
    });
    expect(mic.disabled).toBe(true);
  });

  it("renders mic button enabled when pipeline_health reachable", async () => {
    mockCommands({
      swarm_plan: plannedResponse(),
      swarm_reject: null,
      voice_pipeline_health: { reachable: true, model: "tiny", last_error: null },
    });
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const mic = screen.getByTestId("director-mic") as HTMLButtonElement;
    await waitFor(() => expect(mic.disabled).toBe(false));
    expect(mic.getAttribute("title")).toBe("Push to talk");
  });

  it("clicking mic engages recording state (aria-pressed flips, label shows Stop)", async () => {
    mockCommands({
      swarm_plan: plannedResponse(),
      swarm_reject: null,
      voice_pipeline_health: { reachable: true, model: "tiny", last_error: null },
      transcribe_push_to_talk: {
        text: "hello world",
        language: "en",
        confidence: 0.91,
        latency_ms: 120,
        model: "tiny",
      },
    });
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const mic = await screen.findByTestId("director-mic");
    await waitFor(() => expect((mic as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(mic);
    await waitFor(() => expect(mic.getAttribute("aria-pressed")).toBe("true"));
    expect(mic.textContent).toContain("Stop");
  });

  it("clicking mic again appends transcript to textarea with cursor at end", async () => {
    mockCommands({
      swarm_plan: plannedResponse(),
      swarm_reject: null,
      voice_pipeline_health: { reachable: true, model: "tiny", last_error: null },
      transcribe_push_to_talk: {
        text: "hello world",
        language: "en",
        confidence: 0.91,
        latency_ms: 120,
        model: "tiny",
      },
    });
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const mic = await screen.findByTestId("director-mic");
    await waitFor(() => expect((mic as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(mic);
    fireEvent.click(mic);
    const ta = (await screen.findByTestId(
      "director-textarea",
    )) as HTMLTextAreaElement;
    await waitFor(() => expect(ta.value).toBe("hello world"));
    // Microtask cursor restore.
    await new Promise((r) => setTimeout(r, 0));
    expect(ta.selectionStart).toBe(ta.value.length);
    expect(ta.selectionEnd).toBe(ta.value.length);
  });

  it("backend transcribe error sets the director-error span", async () => {
    mockCommandError("transcribe_push_to_talk", "stt.py failed: bad audio", {
      swarm_plan: plannedResponse(),
      swarm_reject: null,
      voice_pipeline_health: { reachable: true, model: "tiny", last_error: null },
    });
    render(
      <DirectorConsole
        onPlanReady={(): void => {}}
        pendingTicketId={null}
        onPlanCleared={(): void => {}}
      />,
    );
    const mic = await screen.findByTestId("director-mic");
    await waitFor(() => expect((mic as HTMLButtonElement).disabled).toBe(false));
    fireEvent.click(mic);
    fireEvent.click(mic);
    await waitFor(() =>
      expect(screen.getByTestId("director-error")).toHaveTextContent(/bad audio/),
    );
  });
});

describe("appendTranscript helper", () => {
  it("returns the transcript directly when current text is empty", () => {
    expect(appendTranscript("", "hello")).toBe("hello");
  });

  it("joins with a single newline when current text doesn't end in one", () => {
    expect(appendTranscript("foo", "bar")).toBe("foo\nbar");
  });

  it("doesn't add a second newline when current text already ends in one", () => {
    expect(appendTranscript("foo\n", "bar")).toBe("foo\nbar");
  });

  it("preserves multiple existing trailing newlines (no stripping)", () => {
    expect(appendTranscript("foo\n\n", "bar")).toBe("foo\n\nbar");
  });
});
