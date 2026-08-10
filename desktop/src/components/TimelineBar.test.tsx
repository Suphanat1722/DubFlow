import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TimelineBar } from "./TimelineBar";
import type { Cue, SolvedCue } from "../lib/types";

const cues: Cue[] = [
  {
    id: "cue-0001",
    index: 1,
    text: "a",
    srtStartMs: 0,
    srtEndMs: 1000,
    status: "Ready",
    selectedTakeId: "take-1",
    takes: [],
  },
  {
    id: "cue-0002",
    index: 2,
    text: "b",
    srtStartMs: 1500,
    srtEndMs: 2500,
    status: "Too Long",
    selectedTakeId: null,
    takes: [],
  },
];

const solved: SolvedCue[] = [
  {
    cueId: "cue-0001",
    srtStartSample: 0,
    srtEndSample: 48000,
    renderStartMs: 0,
    renderEndMs: 1000,
    renderStartSample: 0,
    renderEndSample: 48000,
    speed: 1,
    status: "Ready",
  },
  {
    cueId: "cue-0002",
    srtStartSample: 72000,
    srtEndSample: 120000,
    renderStartMs: 1500,
    renderEndMs: 3000,
    renderStartSample: 72000,
    renderEndSample: 144000,
    speed: 1.25,
    status: "Too Long",
  },
];

describe("TimelineBar", () => {
  it("renders cue blocks with statuses", () => {
    const { container } = render(
      <TimelineBar
        cues={cues}
        solved={solved}
        videoDurationMs={5000}
        selectedCueId={null}
        onSelectCue={() => {}}
      />,
    );
    const blocks = container.querySelectorAll(".timeline-cue");
    expect(blocks.length).toBe(2);
    expect(blocks[0].className).toContain("cue-Ready");
    expect(blocks[1].className).toContain("cue-Too-Long");
  });

  it("shows legend for all statuses", () => {
    render(
      <TimelineBar
        cues={cues}
        solved={solved}
        videoDurationMs={5000}
        selectedCueId={null}
        onSelectCue={() => {}}
      />,
    );
    expect(screen.getByText((content) => content.includes("พร้อม"))).toBeTruthy();
    expect(screen.getByText((content) => content.includes("ปรับแล้ว"))).toBeTruthy();
    expect(screen.getByText((content) => content.includes("เสียงยาวเกิน"))).toBeTruthy();
    expect(screen.getByText((content) => content.includes("ยังไม่ได้สร้าง"))).toBeTruthy();
  });

  it("calls onSelectCue when a block is clicked", () => {
    let selected: string | null = null;
    const { container } = render(
      <TimelineBar
        cues={cues}
        solved={solved}
        videoDurationMs={5000}
        selectedCueId={null}
        onSelectCue={(id) => (selected = id)}
      />,
    );
    const blocks = container.querySelectorAll(".timeline-cue");
    fireEvent.click(blocks[1]);
    expect(selected).toBe("cue-0002");
  });
});
