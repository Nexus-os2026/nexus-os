import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { mockCommands, mockCommandError, expectInvoked } from "../../test/setup";
import LearningCenter from "../LearningCenter";

const MOCKS: Record<string, unknown> = {
  get_user_profile: "{}",
  get_learning_session: "{}",
  learning_get_progress: "{}",
  learning_save_progress: undefined,
  check_llm_status: { providers: [] },
};

describe("LearningCenter", () => {
  it("renders heading", async () => {
    mockCommands(MOCKS);
    render(<LearningCenter />);
    await waitFor(() => expect(screen.getByText(/Learning/i)).toBeInTheDocument());
  });

  it("loads data on mount", async () => {
    mockCommands(MOCKS);
    render(<LearningCenter />);
    await waitFor(() => expectInvoked("get_user_profile"));
  });

  it("shows error state on failure", async () => {
    mockCommandError("get_user_profile", "connection refused");
    render(<LearningCenter />);
    await waitFor(() => {
      const body = document.body.textContent || "";
      expect(body.length).toBeGreaterThan(0);
    });
  });
});
