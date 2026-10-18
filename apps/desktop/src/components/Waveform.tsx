import type { ClipDto } from "../../../../contracts/ipc";
import { audio } from "../state/audio";
import { seconds } from "../lib/time";
export function Waveform(props: { clip: ClipDto }) {
  const path = () => {
    const p = audio.peaks[props.clip.source.asset];
    if (!p?.values.length) return "M0 30H600";
    const start = (seconds(props.clip.sourceIn) * p.sampleRate) / p.blockFrames;
    const span =
      (seconds(props.clip.duration) *
        seconds(props.clip.speed) *
        p.sampleRate) /
      p.blockFrames;
    const top: string[] = [],
      bottom: string[] = [];
    for (let x = 0; x < 600; x++) {
      const a = Math.floor(start + (x * span) / 600),
        b = Math.ceil(start + ((x + 1) * span) / 600);
      let min = 0,
        max = 0;
      for (let i = a; i < Math.max(a + 1, b) && i < p.values.length; i++) {
        if (i >= 0) {
          min = Math.min(min, p.values[i][0]);
          max = Math.max(max, p.values[i][1]);
        }
      }
      top.push(`${x},${30 - max * 28}`);
      bottom.push(`${x},${30 - min * 28}`);
    }
    return `M${top.join(" L")} L${bottom.reverse().join(" L")} Z`;
  };
  return (
    <svg
      class="waveform"
      viewBox="0 0 600 60"
      preserveAspectRatio="none"
      aria-label={
        audio.peaks[props.clip.source.asset]
          ? "Forma de onda real da fonte"
          : "Fonte sem áudio ou áudio em preparação"
      }
    >
      <path
        d={path()}
        fill="currentColor"
        stroke="currentColor"
        stroke-width=".5"
      />
    </svg>
  );
}
