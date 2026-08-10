import { describe, expect, it } from "vitest";
import { t, setLocale, formatMs, formatSpeed, statusLabel } from "./i18n";

describe("i18n", () => {
  it("returns thai strings for keys", () => {
    expect(t("generateAll")).toBe("สร้างทั้งหมด");
    expect(t("tooLong")).toBe("เสียงยาวเกิน");
    expect(t("appTitle")).toBe("DubFlow");
  });

  it("falls back to thai for unknown locale", () => {
    setLocale("xx");
    expect(t("save")).toBe("บันทึก");
    setLocale("th");
  });

  it("formats milliseconds as timecodes", () => {
    expect(formatMs(0)).toBe("0:00.000");
    expect(formatMs(1_234)).toBe("0:01.234");
    expect(formatMs(65_000)).toBe("1:05.000");
    expect(formatMs(-5)).toBe("0:00.000");
  });

  it("formats speed", () => {
    expect(formatSpeed(1)).toBe("1.00x");
    expect(formatSpeed(1.25)).toBe("1.25x");
  });

  it("maps cue statuses to thai labels", () => {
    expect(statusLabel("Not Generated")).toBe("ยังไม่ได้สร้าง");
    expect(statusLabel("Ready")).toBe("พร้อม");
    expect(statusLabel("Adjusted")).toBe("ปรับแล้ว");
    expect(statusLabel("Too Long")).toBe("เสียงยาวเกิน");
    expect(statusLabel("Error")).toBe("เกิดข้อผิดพลาด");
  });
});
