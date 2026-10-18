import AnimationControls, { KeyButton } from "./AnimationControls";
import type {AnimatedProperty} from "../../../../contracts/ipc";
import { createSignal, Show } from "solid-js";
import { RotateCcw, SlidersHorizontal } from "lucide-solid";
import {
  editor,
  selectedClip,
  canEdit,
  move,
  selectedFrame,
} from "../state/editor";
import { visual, changeVisual, canVisual } from "../state/visual";
import { defaultEffects, defaultTransform } from "../lib/visual";
export function Slider(props: {
  property?:AnimatedProperty;
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onChange: (v: number) => void;
}) {
  const change = (text: string) => {
    const n = Number(text);
    if (text && Number.isFinite(n))
      props.onChange(Math.min(props.max, Math.max(props.min, n)));
  };
  return (
    <label class="parameter">
      <span>
        {props.label}<Show when={props.property}><KeyButton property={props.property!}/></Show>
        <span class="numeric">
          <input
            aria-label={props.label}
            type="number"
            min={props.min}
            max={props.max}
            step={props.step ?? 1}
            value={Number(props.value.toFixed(4))}
            onInput={(e) => change(e.currentTarget.value)}
          />
          {props.unit}
        </span>
      </span>
      <input
        aria-label={`${props.label} slider`}
        type="range"
        min={props.min}
        max={props.max}
        step={props.step ?? 1}
        value={props.value}
        onInput={(e) => change(e.currentTarget.value)}
      />
    </label>
  );
}
export default function Adjustments() {
  const [offset, setOffset] = createSignal("0");
  const t = () => visual().transform,
    fx = () => visual().effects;
  return (
    <section class="panel adjustment-panel">
      <div class="panel-title">
        <h2>{editor.space === "trama" ? "Ajustes de Efeitos" : "Ajustes"}</h2>
        <SlidersHorizontal size={15} />
      </div>
      <div class="selection-label">
        <span class="status-dot" />
        <strong>{selectedClip()?.name ?? "Selecione um clipe de vÃ­deo"}</strong>
        <span class="badge live">RUST</span>
      </div>
      <fieldset class="visual-controls" disabled={!canVisual()}>
        <Show
          when={editor.space === "trama"}
          fallback={
            <>
              <div class="section-caption">Transformar</div>
              <div class="parameter-group">
                <Slider
                  label="Escala" property="scale"
                  value={Math.round(t().scale * 100)}
                  min={1}
                  max={400}
                  unit="%"
                  onChange={(v) => changeVisual({ scale: v / 100 })}
                />
                <Slider
                  label="Opacidade" property="opacity"
                  value={Math.round(t().opacity * 100)}
                  min={0}
                  max={100}
                  unit="%"
                  onChange={(v) => changeVisual({ opacity: v / 100 })}
                />
                <Slider
                  label="PosiÃ§Ã£o X" property="x"
                  value={t().x}
                  min={-1920}
                  max={1920}
                  unit="px"
                  onChange={(v) => changeVisual({ x: v })}
                />
                <Slider
                  label="PosiÃ§Ã£o Y" property="y"
                  value={t().y}
                  min={-1080}
                  max={1080}
                  unit="px"
                  onChange={(v) => changeVisual({ y: v })}
                />
                <Slider label="RotaÃ§Ã£o" property="rotation" value={t().rotation??0} min={-360} max={360} unit="Â°" onChange={rotation=>changeVisual({rotation})}/>
              </div>
            </>
          }
        >
          <div class="section-caption">Ajuste de Cor</div>
          <div class="parameter-group">
            <Slider
              label="Brilho"
              value={fx().brightness}
              min={-1}
              max={1}
              step={0.01}
              onChange={(brightness) => changeVisual({}, { brightness })}
            />
            <Slider
              label="Contraste"
              value={fx().contrast}
              min={0}
              max={2}
              step={0.01}
              onChange={(contrast) => changeVisual({}, { contrast })}
            />
            <Slider
              label="SaturaÃ§Ã£o"
              value={fx().saturation}
              min={0}
              max={2}
              step={0.01}
              onChange={(saturation) => changeVisual({}, { saturation })}
            />
            <button
              class="toggle-row"
              aria-pressed={fx().colorBypass}
              onClick={() =>
                changeVisual({}, { colorBypass: !fx().colorBypass })
              }
            >
              Ignorar cor (Bypass)
              <span class="switch" classList={{ on: fx().colorBypass }} />
            </button>
          </div>
          <div class="section-caption">Desfoque</div>
          <div class="parameter-group">
            <Slider
              label="Raio"
              value={fx().radius}
              min={0}
              max={50}
              unit="px"
              onChange={(radius) => changeVisual({}, { radius })}
            />
            <button
              class="toggle-row"
              aria-pressed={fx().blurBypass}
              onClick={() => changeVisual({}, { blurBypass: !fx().blurBypass })}
            >
              Ignorar desfoque (Bypass)
              <span class="switch" classList={{ on: fx().blurBypass }} />
            </button>
          </div>
        </Show>
        <button
          class="text-button"
          onClick={() =>
            editor.space === "trama"
              ? changeVisual({}, defaultEffects())
              : changeVisual(defaultTransform())
          }
        >
          <RotateCcw size={13} />
          Redefinir parÃ¢metros
        </button>
      </fieldset>
      <AnimationControls/>
      <Show when={editor.space !== "trama"}>
        <div class="section-caption">PosiÃ§Ã£o na timeline</div>
        <div class="move-control">
          <label>
            Deslocamento em quadros
            <input
              aria-label="Deslocamento em quadros"
              type="text"
              inputmode="numeric"
              value={offset()}
              onInput={(e) => setOffset(e.currentTarget.value)}
            />
          </label>
          <button
            disabled={
              !canEdit() || !selectedClip() || !/^[-]?\d+$/.test(offset())
            }
            onClick={() =>
              void move(editor.selected, selectedFrame() + BigInt(offset()))
            }
          >
            Mover
          </button>
        </div>
      </Show>
      <div class="adjustment-bottom">
        <p>
          V1 ao fundo Â· V2 sobreposta.
          <br />
          PosiÃ§Ãµes relativas ao centro de 1920 Ã— 1080.
          <br />
          Ajustes salvos no projeto e na exportaÃ§Ã£o.
        </p>
      </div>
    </section>
  );
}
