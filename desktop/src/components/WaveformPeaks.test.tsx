import { describe, expect, it, beforeAll, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { WaveformPeaks } from "./WaveformPeaks";

// Mock canvas for jsdom
beforeAll(() => {
  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value: (contextId: string) =>
      contextId === "2d"
        ? ({
            clearRect: () => {},
            scale: () => {},
            fillRect: () => {},
          } as unknown as CanvasRenderingContext2D)
        : null,
  });
});

describe("WaveformPeaks", () => {
  afterEach(() => cleanup());

  it("shows loading text when loading", () => {
    render(<WaveformPeaks loading={true} />);
    expect(screen.getByText("กำลังโหลดรูปคลื่น...")).toBeTruthy();
  });

  it("shows empty text when peaks is null", () => {
    render(<WaveformPeaks peaks={null} />);
    expect(screen.getAllByText("ไม่มีเสียงให้แสดง").length).toBeGreaterThanOrEqual(1);
  });

  it("shows empty text when peaks is empty array", () => {
    render(<WaveformPeaks peaks={[]} />);
    expect(screen.getAllByText("ไม่มีเสียงให้แสดง").length).toBeGreaterThanOrEqual(1);
  });

  it("renders a canvas when peaks are provided", () => {
    const peaks = [
      { min: -0.5, max: 0.3 },
      { min: -0.8, max: 0.6 },
      { min: -0.2, max: 0.1 },
    ];
    const { container } = render(<WaveformPeaks peaks={peaks} />);
    const canvas = container.querySelector("canvas");
    expect(canvas).toBeTruthy();
  });
});
