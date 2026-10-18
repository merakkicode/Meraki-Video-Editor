import { expect, test } from "vitest";
import { snapFrame } from "./snap";
test("snap chooses nearest target, both move edges and exact large frames", () => {
  expect(snapFrame(95n, [90n, 97n], 1)).toEqual({ position: 97n, guide: 97n });
  expect(snapFrame(50n, [81n], 1, [0n, 30n])).toEqual({
    position: 51n,
    guide: 81n,
  });
  const n = 90071992547409930n;
  expect(snapFrame(n, [n + 1n], 0.1).position).toBe(n + 1n);
  expect(snapFrame(50n, [52n], 0.1).guide).toBeNull();
  expect(snapFrame(0n, [3n], 1, [10n]).position).toBe(0n);
});
