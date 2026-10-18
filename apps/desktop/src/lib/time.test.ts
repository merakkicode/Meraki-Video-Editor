// @vitest-environment node
import { describe, it, expect } from "vitest";
import { decimal, rational, frameTime, frameAt, timecode, newId } from "./time";
describe("IPC numeric boundaries", () => {
  it("preserves all 128 bits and rejects unsafe syntax", () => {
    const max = (1n << 128n) - 1n;
    expect(decimal(max.toString())).toBe(max);
    for (const text of ["1e6", "+1", "01", "-1", "1.1", " 1", ""])
      expect(() => decimal(text)).toThrow();
    expect(() => decimal((max + 1n).toString())).toThrow();
    expect(decimal(newId())).toBeGreaterThanOrEqual(0n);
  });
  it("uses exact NTSC and audio rational times", () => {
    const rate = { numerator: 30000, denominator: 1001 };
    expect(frameTime(10n, rate)).toEqual(rational(16016n, 48000n));
    const large = 9007199254740993n;
    expect(
      frameAt(frameTime(large, { numerator: 30, denominator: 1 }), {
        numerator: 30,
        denominator: 1,
      }),
    ).toBe(large);
  });
  it("rejects overflow and zero denominator", () => {
    expect(() => rational(1n << 63n, 1n)).toThrow();
    expect(() => rational(1n, 0n)).toThrow();
    expect(rational(0n, 30n)).toEqual({ numerator: "0", denominator: 1 });
    expect(timecode(2520n, { numerator: 30, denominator: 1 })).toBe(
      "00:01:24:00",
    );
  });
});
