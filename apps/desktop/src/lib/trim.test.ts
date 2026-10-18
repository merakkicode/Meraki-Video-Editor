import { test, expect } from "vitest";
import { trimFrameBounds } from "./trim";
import type { ClipDto } from "../../../../contracts/ipc";
test("trim handles use exact speed and rational frame boundaries", () => {
  const t = (n: string, d = 1) => ({ numerator: n, denominator: d });
  const c = {
    timelineStart: t("2"),
    sourceIn: t("3"),
    speed: t("2"),
    source: { availableStart: t("0"), availableDuration: t("10") },
  } as ClipDto;
  expect(trimFrameBounds(c, { numerator: 30, denominator: 1 })).toEqual({
    minHead: 15n,
    maxTail: 165n,
  });
  expect(trimFrameBounds(c, { numerator: 30000, denominator: 1001 })).toEqual({
    minHead: 15n,
    maxTail: 164n,
  });
});
