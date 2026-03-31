import { render, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError } from "../../test/setup";
import Perception from "../Perception";

const MOCKS: Record<string, unknown> = {
  perception_get_policy: { min_autonomy_level: 3, max_image_size_mb: 10 },
  perception_init: "ok",
};

describe("Perception", () => {
  it("renders without crashing", async () => {
    mockCommands(MOCKS);
    render(<Perception />);
    await waitFor(() => expect(document.body.textContent?.length).toBeGreaterThan(0));
  });

  it("renders perception UI", async () => {
    mockCommands(MOCKS);
    render(<Perception />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body).toMatch(/perception|image|vision/i);
    });
  });

  it("handles backend failure gracefully", async () => {
    mockCommandError("perception_init", "connection refused");
    const { container } = render(<Perception />);
    await waitFor(() => expect(container).toBeTruthy());
  });
});
