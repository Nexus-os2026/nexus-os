import { describe, it, expect } from "vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { confirmAction, notify } from "../ownerConfirm";

describe("ownerConfirm", () => {
  it("resolves true only when confirmed", async () => {
    let pending!: Promise<boolean>;
    await act(async () => {
      pending = confirmAction("Delete agent <b>x</b>?", "Delete");
    });
    expect(screen.getByRole("dialog")).toBeTruthy();
    // The message is text, never markup.
    expect(screen.getByText("Delete agent <b>x</b>?")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByText("Delete"));
    });
    await expect(pending).resolves.toBe(true);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("resolves false on cancel and on Escape", async () => {
    let first!: Promise<boolean>;
    await act(async () => {
      first = confirmAction("Proceed?");
    });
    await act(async () => {
      fireEvent.click(screen.getByText("Cancel"));
    });
    await expect(first).resolves.toBe(false);

    let second!: Promise<boolean>;
    await act(async () => {
      second = confirmAction("Proceed?");
    });
    await act(async () => {
      fireEvent.keyDown(window, { key: "Escape" });
    });
    await expect(second).resolves.toBe(false);
  });

  it("shows a notice until acknowledged", async () => {
    let done!: Promise<void>;
    await act(async () => {
      done = notify("Shared 3 strategies.");
    });
    expect(screen.getByText("Shared 3 strategies.")).toBeTruthy();
    await act(async () => {
      fireEvent.click(screen.getByText("OK"));
    });
    await expect(done).resolves.toBeUndefined();
  });
});
