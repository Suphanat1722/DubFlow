import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { StatusBar } from "./StatusBar";

describe("StatusBar", () => {
  it("shows checking state", () => {
    render(<StatusBar runtimeState={{ kind: "checking" }} onInitRuntime={() => {}} />);
    expect(screen.getByText("กำลังโหลด...")).toBeTruthy();
  });

  it("shows worker-not-running with init button", () => {
    render(
      <StatusBar runtimeState={{ kind: "no-worker" }} onInitRuntime={() => {}} />,
    );
    expect(screen.getByText("Worker ยังไม่เริ่มทำงาน")).toBeTruthy();
    expect(screen.getByText("เริ่ม Runtime")).toBeTruthy();
  });

  it("shows error message", () => {
    render(
      <StatusBar
        runtimeState={{ kind: "error", message: "CUDA fail" }}
        onInitRuntime={() => {}}
      />,
    );
    expect(screen.getByText(/CUDA fail/)).toBeTruthy();
  });

  it("calls onInitRuntime from init button", () => {
    const onInit = vi.fn();
    render(
      <StatusBar runtimeState={{ kind: "worker-running" }} onInitRuntime={onInit} />,
    );
    fireEvent.click(screen.getByText("เริ่ม Runtime"));
    expect(onInit).toHaveBeenCalled();
  });

  it("shows tts-ready with device", () => {
    render(
      <StatusBar
        runtimeState={{ kind: "tts-ready", device: "cuda" }}
        onInitRuntime={() => {}}
      />,
    );
    expect(screen.getByText(/โมเดลเสียงพร้อมใช้งาน \(cuda\)/)).toBeTruthy();
  });
});
