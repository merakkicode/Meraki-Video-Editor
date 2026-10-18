import { createEffect, createSignal, For, Show } from "solid-js";
import { Headphones, Play, Pause } from "lucide-solid";
import { editor, togglePlayback, report } from "../state/editor";
import {
  audio,
  setAudio,
  channel,
  changeChannel,
  eqResponse,
} from "../state/audio";
const bands = [
  { name: "Graves", hz: 120, color: "#00d2ff" },
  { name: "Médios", hz: 1000, color: "#6292ee" },
  { name: "Agudos", hz: 8000, color: "#9d4edd" },
];
export function Equalizer() {
  const [response, setResponse] = createSignal<[number, number][]>([]);
  let request = 0;
  createEffect(() => {
    const eq = [...channel(audio.selected).eqDb] as [number, number, number];
    const id = ++request;
    if (editor.connected)
      void eqResponse(eq)
        .then((r) => {
          if (id === request) setResponse(r);
        })
        .catch(report);
  });
  const curve = () =>
    response()
      .map(
        ([hz, db], i) =>
          `${i ? "L" : "M"}${(Math.log10(hz / 20) / 3) * 700},${140 - db * 8}`,
      )
      .join(" ");
  const gain = (i: number, value: number) => {
    const eq = [...channel(audio.selected).eqDb] as [number, number, number];
    eq[i] = Math.max(-12, Math.min(12, value));
    changeChannel(audio.selected, { eqDb: eq });
  };
  function drag(e: PointerEvent, i: number) {
    const el = e.currentTarget as SVGCircleElement;
    const r = el.ownerSVGElement!.getBoundingClientRect();
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) =>
      gain(
        i,
        Math.round(((140 - ((ev.clientY - r.top) / r.height) * 280) / 8) * 10) /
          10,
      );
    const stop = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", stop);
      el.removeEventListener("pointercancel", stop);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", stop);
    el.addEventListener("pointercancel", stop);
  }
  return (
    <section class="panel eq-panel">
      <div class="panel-title">
        <h2>Equalizador Paramétrico</h2>
        <button
          aria-label={editor.playing ? "Pausar" : "Reproduzir"}
          onClick={togglePlayback}
        >
          {editor.playing ? <Pause size={16} /> : <Play size={16} />}
        </button>
      </div>
      <label class="audio-target">
        Canal{" "}
        <select
          value={audio.selected}
          onChange={(e) => setAudio("selected", e.currentTarget.value)}
        >
          <option value="master">Saída Principal</option>
          <For each={editor.snapshot.timeline.tracks}>
            {(t) => (
              <option value={t.id}>
                {t.name}
                {t.kind === "video" ? " · áudio embutido" : ""}
              </option>
            )}
          </For>
        </select>
      </label>
      <div class="eq-chart">
        <div class="eq-scale">
          <span>+12</span>
          <span>0 dB</span>
          <span>−12</span>
        </div>
        <svg
          viewBox="0 0 700 280"
          preserveAspectRatio="none"
          aria-label="Resposta real dos três filtros biquad"
        >
          <defs>
            <linearGradient id="eq-gradient" gradientUnits="userSpaceOnUse" x1="0" x2="700">
              <stop stop-color="#00d2ff" />
              <stop offset="1" stop-color="#9d4edd" />
            </linearGradient>
          </defs>
          <For each={[0, 70, 140, 210, 280]}>
            {(y) => <line x1="0" x2="700" y1={y} y2={y} class="eq-grid" />}
          </For>
          <path
            d={curve()}
            stroke="url(#eq-gradient)"
            fill="none"
            stroke-width="3"
          />
          <For each={bands}>
            {(b, i) => (
              <circle
                cx={(Math.log10(b.hz / 20) / 3) * 700}
                cy={140 - channel(audio.selected).eqDb[i()] * 8}
                r="7"
                fill={b.color}
                stroke="white"
                tabindex="0"
                role="slider"
                aria-label={b.name}
                aria-valuemin={-12}
                aria-valuemax={12}
                aria-valuenow={channel(audio.selected).eqDb[i()]}
                onPointerDown={(e) => drag(e, i())}
                onKeyDown={(e) => {
                  if (e.key === "ArrowUp" || e.key === "ArrowDown") {
                    e.preventDefault();
                    gain(
                      i(),
                      channel(audio.selected).eqDb[i()] +
                        (e.key === "ArrowUp" ? 1 : -1),
                    );
                  }
                }}
              />
            )}
          </For>
        </svg>
        <div class="eq-frequency">
          <span>20 Hz</span>
          <span>100</span>
          <span>1k</span>
          <span>10k</span>
          <span>20k Hz</span>
        </div>
      </div>
      <div class="eq-controls">
        <For each={bands}>
          {(b, i) => (
            <label class="knob-control">
              <div
                class="knob"
                style={{
                  "--knob-color": b.color,
                  "--knob-angle": `${channel(audio.selected).eqDb[i()] * 8}deg`,
                }}
              >
                <span />
              </div>
              <input
                aria-label={`Ganho ${b.name}`}
                type="range"
                min="-12"
                max="12"
                step=".1"
                value={channel(audio.selected).eqDb[i()]}
                onInput={(e) => gain(i(), +e.currentTarget.value)}
              />
              <strong>{b.name}</strong>
              <output>{channel(audio.selected).eqDb[i()].toFixed(1)} dB</output>
              <small>{b.hz} Hz</small>
            </label>
          )}
        </For>
      </div>
      <div class="panel-footer">Low-shelf · Peaking Q 0,707 · High-shelf</div>
    </section>
  );
}
export function Mixer() {
  const channels = () => [
    ...editor.snapshot.timeline.tracks.map((t) => ({ id: t.id, name: t.name })),
    { id: "master", name: "Saída Principal" },
  ];
  const meter = (id: string) => audio.state.meters.find((m) => m.id === id);
  const height = (db: number) =>
    `${Math.max(0, Math.min(100, ((db + 60) / 60) * 100))}%`;
  return (
    <section class="panel mixer-panel">
      <div class="panel-title">
        <h2>Mesa</h2>
        <Headphones size={15} />
      </div>
      <div class="mixer-channels">
        <For each={channels()}>
          {(c) => (
            <div
              class="mixer-channel"
              classList={{
                master: c.id === "master",
                selected: audio.selected === c.id,
              }}
            >
              <button
                class="channel-name"
                onClick={() => setAudio("selected", c.id)}
              >
                {c.name}
              </button>
              <div class="fader-well">
                <div class="meter-scale">
                  <span>0</span>
                  <span>−12</span>
                  <span>−24</span>
                  <span>−60</span>
                </div>
                <div class="stereo-meter" aria-label={`Picos ${c.name}`}>
                  <For each={[0, 1]}>
                    {(i) => (
                      <div>
                        <span
                          style={{
                            height: "100%",
                            "clip-path": `inset(${100 - parseFloat(height(meter(c.id)?.peak[i] ?? -120))}% 0 0 0)`,
                          }}
                        />
                        <i
                          class="rms-marker"
                          style={{
                            bottom: height(meter(c.id)?.rms[i] ?? -120),
                          }}
                        />
                      </div>
                    )}
                  </For>
                </div>
                <input
                  aria-label={`Volume ${c.name}`}
                  class="fader"
                  type="range"
                  min="-61"
                  max="6"
                  step=".5"
                  value={channel(c.id).gainDb ?? -61}
                  onInput={(e) =>
                    changeChannel(c.id, {
                      gainDb:
                        +e.currentTarget.value < -60
                          ? null
                          : +e.currentTarget.value,
                    })
                  }
                />
              </div>
              <output>
                {channel(c.id).gainDb === null
                  ? "−∞"
                  : channel(c.id).gainDb!.toFixed(1)}
                <small> dB</small>
              </output>
              <small class="meter-readout">
                P {Math.max(...(meter(c.id)?.peak ?? [-120])).toFixed(1)} dBFS
              </small>
              <div class="mixer-buttons">
                <button
                  aria-label={`Mute ${c.name}`}
                  aria-pressed={channel(c.id).muted}
                  classList={{ active: channel(c.id).muted }}
                  onClick={() =>
                    changeChannel(c.id, { muted: !channel(c.id).muted })
                  }
                >
                  M
                </button>
                <Show when={c.id !== "master"}>
                  <button
                    aria-label={`Solo ${c.name}`}
                    aria-pressed={channel(c.id).solo}
                    classList={{ active: channel(c.id).solo }}
                    onClick={() =>
                      changeChannel(c.id, { solo: !channel(c.id).solo })
                    }
                  >
                    S
                  </button>
                </Show>
              </div>
            </div>
          )}
        </For>
      </div>
      <div class="panel-footer audio-status" role="status">
        {audio.state.preparing
          ? "Preparando PCM e formas de onda…"
          : (audio.state.error ?? audio.state.device)}
        <br />
        Latência estimada {audio.state.latencyMs.toFixed(1)} ms · underruns{" "}
        {audio.state.underruns}
      </div>
    </section>
  );
}
