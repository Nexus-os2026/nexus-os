import { render } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Workflows } from "../Workflows";

describe("Workflows", () => {
  it("renders without crashing", () => {
    const { container } = render(<Workflows />);
    expect(container).toBeTruthy();
    expect(container.innerHTML.length).toBeGreaterThan(0);
  });
});
