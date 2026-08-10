import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SubtitleList } from "./SubtitleList";
import type { Cue } from "../lib/types";

const cues: Cue[] = [
  {
    id: "cue-0001",
    index: 1,
    text: "สวัสดีครับ",
    srtStartMs: 1000,
    srtEndMs: 3000,
    status: "Ready",
    selectedTakeId: "take-001",
    takes: [
      {
        takeId: "take-001",
        cueId: "cue-0001",
        provider: "jaitts-f5tts",
        providerVersion: "1.1.22",
        seed: 42,
        durationMs: 2000,
        settingsHash: "abc",
        audioPath: "takes/take-001.wav",
      },
    ],
  },
  {
    id: "cue-0002",
    index: 2,
    text: "ลาก่อน",
    srtStartMs: 4000,
    srtEndMs: 6000,
    status: "Not Generated",
    selectedTakeId: null,
    takes: [],
  },
];

describe("SubtitleList", () => {
  it("shows an empty state when no cues", () => {
    render(
      <SubtitleList
        cues={[]}
        selectedCueId={null}
        onSelectCue={() => {}}
        onTextChange={() => {}}
        onGenerateOne={() => {}}
        onRegenerateOne={() => {}}
        onPlay={() => {}}
        busyCueIds={new Set()}
      />,
    );
    expect(screen.getByText("ไม่มีคำบรรยายในโปรเจกต์นี้")).toBeTruthy();
  });

  it("renders cue rows with thai status", () => {
    render(
      <SubtitleList
        cues={cues}
        selectedCueId={null}
        onSelectCue={() => {}}
        onTextChange={() => {}}
        onGenerateOne={() => {}}
        onRegenerateOne={() => {}}
        onPlay={() => {}}
        busyCueIds={new Set()}
      />,
    );
    expect(screen.getByText("สวัสดีครับ")).toBeTruthy();
    expect(screen.getByText("ลาก่อน")).toBeTruthy();
    expect(screen.getByText("พร้อม")).toBeTruthy();
    expect(screen.getByText("ยังไม่ได้สร้าง")).toBeTruthy();
  });

  it("calls onSelectCue when row clicked", () => {
    let selected: string | null = null;
    render(
      <SubtitleList
        cues={cues}
        selectedCueId={null}
        onSelectCue={(id) => (selected = id)}
        onTextChange={() => {}}
        onGenerateOne={() => {}}
        onRegenerateOne={() => {}}
        onPlay={() => {}}
        busyCueIds={new Set()}
      />,
    );
    fireEvent.click(screen.getByTestId("cue-row-cue-0001"));
    expect(selected).toBe("cue-0001");
  });

  it("shows generate button for cue without take", () => {
    render(
      <SubtitleList
        cues={cues}
        selectedCueId={null}
        onSelectCue={() => {}}
        onTextChange={() => {}}
        onGenerateOne={() => {}}
        onRegenerateOne={() => {}}
        onPlay={() => {}}
        busyCueIds={new Set()}
      />,
    );
    expect(screen.getAllByTitle("สร้างเสียง").length).toBe(1);
  });

  it("shows busy label for generating cues", () => {
    render(
      <SubtitleList
        cues={cues}
        selectedCueId={null}
        onSelectCue={() => {}}
        onTextChange={() => {}}
        onGenerateOne={() => {}}
        onRegenerateOne={() => {}}
        onPlay={() => {}}
        busyCueIds={new Set(["cue-0002"])}
      />,
    );
    expect(screen.getAllByText("กำลังสร้าง").length).toBeGreaterThanOrEqual(1);
  });
});
