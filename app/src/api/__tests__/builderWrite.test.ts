import { describe, expect, it } from "vitest";
import { builderDevServerWriteFile } from "../backend";
import { mockCommandError, mockCommands, mockInvoke } from "../../test/setup";

describe("Builder existing-project write contract", () => {
  it("sends only selector, relative path and content, including compatibility key aliases", async () => {
    mockCommands({ builder_dev_server_write_file: undefined });
    const caller = builderDevServerWriteFile as (...args: unknown[]) => Promise<void>;
    await expect(caller("project", "src/App.tsx", "content", {
      root: "/outside", projectDir: "/outside", grant: "forged", binding: "forged", runId: "forged",
    })).resolves.toBeUndefined();
    expect(mockInvoke).toHaveBeenCalledWith("builder_dev_server_write_file", {
      projectId: "project", project_id: "project",
      relativePath: "src/App.tsx", relative_path: "src/App.tsx", content: "content",
    });
  });

  it("propagates backend authority failures", async () => {
    mockCommandError("builder_dev_server_write_file", "Builder write: project not registered");
    await expect(builderDevServerWriteFile("legacy", "App.tsx", "content"))
      .rejects.toThrow("Builder write: project not registered");
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });
});
