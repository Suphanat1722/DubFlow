import { beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ExportPanel } from "./ExportPanel";

vi.mock("@tauri-apps/api/core", () => {
  return {
    invoke: vi.fn(),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => {
  return {
    save: vi.fn(),
  };
});

import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

const mockedInvoke = invoke as unknown as ReturnType<typeof vi.fn>;
const mockedSave = save as unknown as ReturnType<typeof vi.fn>;

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedSave.mockReset();
});

function renderPanel() {
  return render(
    <ExportPanel
      projectDir="C:\\proj"
      onMessage={() => {}}
      onError={() => {}}
    />,
  );
}

describe("ExportPanel", () => {
  it("shows three export modes and defaults to replace", () => {
    renderPanel();
    expect(screen.getByTestId("export-mode-replace")).toBeTruthy();
    expect(screen.getByTestId("export-mode-mix")).toBeTruthy();
    expect(screen.getByTestId("export-mode-voiceTrack")).toBeTruthy();
  });

  it("disables export when validation reports a blocker", async () => {
    mockedInvoke.mockResolvedValueOnce({
      exportBlocked: true,
      reasons: ["cue 1 (a): still not generated"],
    });
    renderPanel();
    await waitFor(() => {
      expect(screen.getByTestId("export-blocked")).toBeTruthy();
    });
    expect(screen.getByTestId("export-run").hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/still not generated/)).toBeTruthy();
  });

  it("runs export and reports the output path", async () => {
    mockedInvoke
      .mockResolvedValueOnce({ exportBlocked: false, reasons: [] });
    mockedInvoke.mockResolvedValueOnce({
      samples: 96000,
      outputPath: "C:\\out.mp4",
    });
    mockedSave.mockResolvedValueOnce("C:\\out.mp4");

    const onMessage = vi.fn();
    render(
      <ExportPanel
        projectDir="C:\\proj"
        onMessage={onMessage}
        onError={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("export-run").hasAttribute("disabled")).toBe(false);
    });
    fireEvent.click(screen.getByTestId("export-run"));

    await waitFor(() => {
      expect(screen.getByTestId("export-result")).toBeTruthy();
    });
    expect(screen.getByText(/C:\\out\.mp4/)).toBeTruthy();
    expect(onMessage).toHaveBeenCalledWith(expect.stringContaining("Export"));
  });

  it("switches to mix mode and shows gain control", async () => {
    mockedInvoke.mockResolvedValue({ exportBlocked: false, reasons: [] });
    renderPanel();
    fireEvent.click(screen.getByTestId("export-mode-mix"));
    await waitFor(() => {
      expect(screen.getByTestId("mix-gain")).toBeTruthy();
    });
  });
});
