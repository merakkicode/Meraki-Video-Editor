import type { TimeDto, TimelineDto } from "../../../../contracts/ipc";
export type Rate = TimelineDto["frameRate"];
const I64_MIN = -(1n << 63n),
  I64_MAX = (1n << 63n) - 1n;
export function decimal(text: string, max = (1n << 128n) - 1n): bigint {
  if (!/^(0|[1-9]\d*)$/.test(text))
    throw new Error("Inteiro decimal inválido.");
  const value = BigInt(text);
  if (value > max) throw new Error("Inteiro fora do intervalo.");
  return value;
}
export function rational(num: bigint, den: bigint): TimeDto {
  if (den <= 0n) throw new Error("Denominador inválido.");
  let a = num < 0n ? -num : num,
    b = den;
  while (b) [a, b] = [b, a % b];
  num /= a;
  den /= a;
  if (num < I64_MIN || num > I64_MAX || den > 4294967295n)
    throw new Error("Tempo fora do intervalo.");
  return { numerator: num.toString(), denominator: Number(den) };
}
export function frameTime(frame: bigint, rate: Rate): TimeDto {
  return rational(frame * BigInt(rate.denominator), BigInt(rate.numerator));
}
export function frameAt(time: TimeDto, rate: Rate): bigint {
  return (
    (BigInt(time.numerator) * BigInt(rate.numerator)) /
    (BigInt(time.denominator) * BigInt(rate.denominator))
  );
}
/** Approximation for screen geometry only. Never use this result to serialize an edit. */
export function seconds(time: TimeDto): number {
  return Number(time.numerator) / time.denominator;
}
export function timecode(frame: bigint, rate: Rate): string {
  const fps = BigInt(Math.round(rate.numerator / rate.denominator));
  const positive = frame < 0n ? 0n : frame;
  const parts = [
    positive / fps / 3600n,
    (positive / fps / 60n) % 60n,
    (positive / fps) % 60n,
    positive % fps,
  ];
  return parts.map((n) => n.toString().padStart(2, "0")).join(":"); // NDF. Drop-frame formatting is not claimed by this shell.
}
export function newId(): string {
  const values = crypto.getRandomValues(new Uint32Array(4));
  return values.reduce((n, v) => (n << 32n) | BigInt(v), 0n).toString();
}
