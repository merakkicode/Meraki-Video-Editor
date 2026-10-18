import type { ClipDto, TimeDto } from "../../../../contracts/ipc";
import type { Rate } from "./time";
type Q = [bigint, bigint];
const q = (t: TimeDto): Q => [BigInt(t.numerator), BigInt(t.denominator)];
const add = (a: Q, b: Q): Q => [a[0] * b[1] + b[0] * a[1], a[1] * b[1]];
const sub = (a: Q, b: Q): Q => add(a, [-b[0], b[1]]);
const floor = (n: bigint, d: bigint) =>
  n / d - (n < 0n && n % d !== 0n ? 1n : 0n);
export function trimFrameBounds(c: ClipDto, rate: Rate) {
  const speed = q(c.speed),
    source = q(c.sourceIn),
    start = q(c.timelineStart),
    available = q(c.source.availableStart);
  const unscale = (v: Q): Q => [v[0] * speed[1], v[1] * speed[0]];
  const earliest = sub(start, unscale(sub(source, available)));
  const latest = add(
    start,
    unscale(sub(add(available, q(c.source.availableDuration)), source)),
  );
  const frame = (v: Q): Q => [
    v[0] * BigInt(rate.numerator),
    v[1] * BigInt(rate.denominator),
  ];
  const [a, b] = frame(earliest),
    [x, y] = frame(latest);
  return { minHead: -floor(-a, b), maxTail: floor(x, y) };
}
