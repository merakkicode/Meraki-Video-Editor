/** All arithmetic is in integral project frames. Pixels only determine the magnetic radius. */
export function snapFrame(
  frame: bigint,
  targets: readonly bigint[],
  framesPerPixel: number,
  offsets: readonly bigint[] = [0n],
) {
  const radius = BigInt(Math.max(0, Math.floor(10 * framesPerPixel)));
  let distance = radius + 1n,
    position = frame,
    guide: bigint | null = null;
  for (const offset of offsets)
    for (const target of targets) {
      const candidate = target - offset;
      if (candidate < 0n) continue;
      const d = candidate > frame ? candidate - frame : frame - candidate;
      if (d <= radius && d < distance) {
        distance = d;
        position = candidate;
        guide = target;
      }
    }
  return { position, guide };
}
