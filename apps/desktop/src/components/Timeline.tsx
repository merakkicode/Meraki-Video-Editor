import { trimFrameBounds } from "../lib/trim";
import { workflow, mediaContext } from "../state/workflow";
import { snapFrame } from "../lib/snap";
import { addMarker, setEditingMarker } from "./MarkerDialog";
import { frameTime } from "../lib/time";
import { edit } from "../state/editor";
import { createTitle } from "../state/editor";
import { createSignal, For, Show } from "solid-js";
import {
  MousePointer2,
  Scissors,
  Magnet,
  Undo2,
  LockKeyhole,
  UnlockKeyhole,
  Volume2,
  Eye,
  Minus,
  Plus,
  Flag,
  Keyboard,
  ChevronDown,
} from "lucide-solid";
import type { ClipDto } from "../../../../contracts/ipc";
import {
  editor,
  setEditor,
  rate,
  select,
  seek,
  split,
  move,
  undo,
  canEdit,
  toggleTrack,
} from "../state/editor";
import { frameAt, seconds, timecode } from "../lib/time";
import { Waveform } from "./Waveform";
import { channel } from "../state/audio";
export default function Timeline(props: { audio?: boolean }) {
  let lanes!: HTMLDivElement;
  const [ghost, setGhost] = createSignal<{
    id: string;
    x: number;
    width?: number;
  } | null>(null);
  const duration = () =>
    Math.max(
      15,
      ...editor.snapshot.timeline.tracks.flatMap((t) =>
        t.clips.map((c) => seconds(c.timelineStart) + seconds(c.duration) + 10),
      ),
    );
  const tracks = () => editor.snapshot.timeline.tracks;
  const rulerStep = () => (duration() <= 30 ? 2 : duration() <= 90 ? 5 : 15);
  const projectFrames = () =>
    Math.ceil((duration() * rate().numerator) / rate().denominator);
  const atX = (clientX: number) => {
    const r = lanes.getBoundingClientRect();
    return BigInt(
      Math.max(0, Math.round(((clientX - r.left) / r.width) * projectFrames())),
    );
  };
  const [guide, setGuide] = createSignal<bigint | null>(null);
  const targets = (exclude?: string, playhead = true) => [
    0n,
    ...(playhead ? [editor.frame] : []),
    ...(editor.snapshot.timeline.markers ?? []).map((m) =>
      frameAt(m.at, rate()),
    ),
    ...tracks().flatMap((t) =>
      t.clips
        .filter((c) => c.id !== exclude)
        .flatMap((c) => [
          frameAt(c.timelineStart, rate()),
          frameAt(c.timelineStart, rate()) + frameAt(c.duration, rate()),
        ]),
    ),
  ];
  function magnet(
    frame: bigint,
    exclude?: string,
    offsets = [0n],
    playhead = true,
  ) {
    if (!editor.snap) {
      setGuide(null);
      return frame;
    }
    const result = snapFrame(
      frame,
      targets(exclude, playhead),
      projectFrames() / lanes.getBoundingClientRect().width,
      offsets,
    );
    setGuide(result.guide);
    return result.position;
  }
  function gesture(
    e: PointerEvent,
    update: (e: PointerEvent) => void,
    commit: () => void,
  ) {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const clear = () => {
      setGhost(null);
      setGuide(null);
      el.removeEventListener("pointermove", update);
      el.removeEventListener("pointerup", finish);
      el.removeEventListener("pointercancel", cancel);
    };
    const finish = () => {
        clear();
        commit();
      },
      cancel = () => clear();
    el.addEventListener("pointermove", update);
    el.addEventListener("pointerup", finish);
    el.addEventListener("pointercancel", cancel);
  }
  function scrub(e: PointerEvent) {
    if (e.button !== 0) return;
    const update = (ev: PointerEvent) =>
      seek(magnet(atX(ev.clientX), undefined, [0n], false));
    update(e);
    gesture(e, update, () => {});
  }
  function drag(e: PointerEvent, c: ClipDto, edge?: "head" | "tail") {
    e.stopPropagation();
    if (e.button !== 0) return;
    select(c.id);
    if (!edge && editor.tool === "trim") {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      edge = e.clientX < r.left + r.width / 2 ? "head" : "tail";
    }
    if (!edge && editor.tool === "cut") {
      void split(c.id, magnet(atX(e.clientX)));
      setGuide(null);
      return;
    }
    if (!canEdit()) return;
    const track = tracks().find((t) => t.clips.some((x) => x.id === c.id));
    if (track && (track.locked || editor.trackControls[track.id]?.locked))
      return;
    const start = e.clientX,
      base = frameAt(c.timelineStart, rate()),
      length = frameAt(c.duration, rate()),
      end = base + length;
    let destination = edge === "tail" ? end : base;
    const width = lanes.getBoundingClientRect().width;
    const update = (ev: PointerEvent) => {
      const delta = BigInt(
        Math.round(((ev.clientX - start) / width) * projectFrames()),
      );
      destination = magnet(
        (edge === "tail" ? end : base) + delta,
        c.id,
        edge ? [0n] : [0n, length],
      );
      if (destination < 0n) destination = 0n;
      if (edge) {
        const bounds = trimFrameBounds(c, rate());
        let min = bounds.minHead > 0n ? bounds.minHead : 0n,
          max = bounds.maxTail;
        if (!e.shiftKey && track) {
          for (const other of track.clips) {
            if (other.id === c.id) continue;
            const begin = frameAt(other.timelineStart, rate()),
              finish = begin + frameAt(other.duration, rate());
            if (finish <= base && finish > min) min = finish;
            if (begin >= end && begin < max) max = begin;
          }
        }
        if (edge === "head") {
          if (destination < min) destination = min;
          if (destination >= end) destination = end - 1n;
        } else {
          if (!c.motion?.title && destination > max) destination = max;
          if (destination <= base) destination = base + 1n;
        }
      }
      if (guide() !== null && edge && guide() !== destination) setGuide(null);
      const x = edge === "tail" ? base : destination,
        w =
          edge === "head"
            ? end - destination
            : edge === "tail"
              ? destination - base
              : length;
      setGhost({
        id: c.id,
        x: (Number(x) / projectFrames()) * 100,
        width: (Number(w) / projectFrames()) * 100,
      });
    };
    gesture(e, update, () => {
      if (destination === (edge === "tail" ? end : base)) return;
      if (edge)
        void edit({
          kind: "trim",
          clip: c.id,
          head: edge === "head",
          at: frameTime(destination, rate()),
          ripple: e.shiftKey,
        });
      else void move(c.id, destination);
    });
  }
  const position = (c: ClipDto) =>
    ghost()?.id === c.id
      ? ghost()!.x
      : (seconds(c.timelineStart) / duration()) * 100;
  return (
    <section class="panel timeline-panel">
      <div class="timeline-toolbar">
        <div class="timeline-heading">
          Linha do Tempo <ChevronDown size={13} />
        </div>
        <div class="timeline-tools">
          <button
            classList={{ active: editor.tool === "select" }}
            title="Seleção (V)"
            onClick={() => setEditor("tool", "select")}
          >
            <MousePointer2 size={15} />
            <span>Seleção</span>
          </button>
          <button
            classList={{ active: editor.tool === "cut" }}
            title="Corte (C): clique em um clipe"
            onClick={() => setEditor("tool", "cut")}
          >
            <Scissors size={15} />
            <span>Corte</span>
          </button>
          <button
            title="Arraste as bordas; Shift ativa Ripple nesta trilha"
            classList={{ active: editor.tool === "trim" }}
            onClick={() => setEditor("tool", "trim")}
          >
            <span>↦↤</span>
            <span>Aparar</span>
          </button>
          <button
            title="Criar título em V2 na agulha (T)"
            disabled={!canEdit()}
            onClick={() => void createTitle()}
          >
            T Texto
          </button>
          <button
            title="Adicionar marcador (M)"
            disabled={!canEdit()}
            onClick={addMarker}
          >
            <Flag size={15} />
            Marcador
          </button>
          <span class="tool-divider" />
          <button
            classList={{ active: editor.snap }}
            title="Encaixe (N)"
            aria-pressed={editor.snap}
            onClick={() => setEditor("snap", !editor.snap)}
          >
            <Magnet size={15} />
            <span>Encaixe</span>
          </button>
          <button
            title="Cortar selecionado no playhead (Ctrl+B)"
            disabled={!canEdit()}
            onClick={() => void split()}
          >
            <Scissors size={15} />
            <span>Dividir</span>
          </button>
          <button
            title="Desfazer (Ctrl+Z)"
            disabled={!canEdit() || editor.snapshot.revision === "0"}
            onClick={() => void undo()}
          >
            <Undo2 size={15} />
          </button>
        </div>
        <output class="timeline-time">{timecode(editor.frame, rate())}</output>
        <div class="zoom">
          <Minus size={12} />
          <input
            aria-label="Zoom da timeline"
            type="range"
            min="1"
            max="3"
            step=".1"
            value={editor.zoom}
            onInput={(e) => setEditor("zoom", Number(e.currentTarget.value))}
          />
          <Plus size={12} />
        </div>
      </div>
      <div class="timeline-scroll">
        <div
          class="timeline-content"
          style={{ width: `${editor.zoom * 100}%` }}
        >
          <div class="track-headings">
            <div class="track-corner">
              <span>{props.audio ? "ÁUDIO" : "SEQUÊNCIA 01"}</span>
              <Flag size={13} />
            </div>
            <For each={tracks()}>
              {(track) => (
                <div
                  class={`track-header ${track.kind}`}
                  style={{ height: props.audio ? "104px" : "68px" }}
                >
                  <div class="track-name">
                    <span>
                      {track.kind === "video" ? (
                        <Eye size={13} />
                      ) : (
                        <Volume2 size={13} />
                      )}
                    </span>
                    <strong>{track.name}</strong>
                    <span class="track-code">
                      {track.kind === "video" ? "V" : "A"}
                      {tracks()
                        .filter((t) => t.kind === track.kind)
                        .findIndex((t) => t.id === track.id) + 1}
                    </span>
                  </div>
                  <div class="track-buttons">
                    <button
                      title="Silenciar áudio da trilha"
                      classList={{
                        active: channel(track.id).muted,
                      }}
                      onClick={() => toggleTrack(track.id, "muted")}
                    >
                      M
                    </button>
                    <button
                      title="Isolar áudio da trilha"
                      classList={{
                        active: channel(track.id).solo,
                      }}
                      onClick={() => toggleTrack(track.id, "solo")}
                    >
                      S
                    </button>
                    <button
                      title="Bloqueio de edição nesta sessão"
                      classList={{
                        active:
                          editor.trackControls[track.id]?.locked ||
                          track.locked,
                      }}
                      onClick={() => toggleTrack(track.id, "locked")}
                    >
                      {editor.trackControls[track.id]?.locked ||
                      track.locked ? (
                        <LockKeyhole size={12} />
                      ) : (
                        <UnlockKeyhole size={12} />
                      )}
                    </button>
                  </div>
                </div>
              )}
            </For>
          </div>
          <div
            class="timeline-lanes"
            ref={(el) => {
              lanes = el;
            }}
          >
            <div class="time-ruler" onPointerDown={scrub}>
              <For each={editor.snapshot.timeline.markers ?? []}>
                {(m) => (
                  <button
                    class={`timeline-marker marker-${m.color}`}
                    title={m.name}
                    aria-label={`Marcador ${m.name}`}
                    style={{
                      left: `${(Number(frameAt(m.at, rate())) / projectFrames()) * 100}%`,
                    }}
                    onPointerDown={(e) => {
                      e.stopPropagation();
                      seek(frameAt(m.at, rate()));
                    }}
                    onDblClick={() => setEditingMarker(m)}
                  >
                    ⚑
                  </button>
                )}
              </For>
              <For
                each={Array.from(
                  { length: Math.ceil(duration() / rulerStep()) + 1 },
                  (_, i) => i * rulerStep(),
                )}
              >
                {(s) => (
                  <span style={{ left: `${(s / duration()) * 100}%` }}>
                    {Math.floor(s / 60)
                      .toString()
                      .padStart(2, "0")}
                    :{(s % 60).toString().padStart(2, "0")}
                  </span>
                )}
              </For>
            </div>
            <For each={tracks()}>
              {(track) => (
                <div
                  class="track-lane"
                  style={{ height: props.audio ? "104px" : "68px" }}
                  onPointerDown={scrub}
                >
                  <For each={track.clips}>
                    {(clip) => (
                      <div
                        role="button"
                        tabindex="0"
                        aria-label={`Clipe ${clip.name}`}
                        class={`timeline-clip ${track.kind}`}
                        classList={{
                          selected: editor.selected === clip.id,
                          offline:
                            !clip.motion?.title &&
                            workflow.offline.includes(clip.source.asset),
                          dragging: ghost()?.id === clip.id,
                          muted: editor.trackControls[track.id]?.muted,
                        }}
                        style={{
                          left: `${position(clip)}%`,
                          width: `${ghost()?.id === clip.id ? (ghost()!.width ?? (seconds(clip.duration) / duration()) * 100) : (seconds(clip.duration) / duration()) * 100}%`,
                        }}
                        onContextMenu={(e) => {
                          if (!clip.motion?.title)
                            mediaContext(e, clip.source.asset);
                        }}
                        onPointerDown={(e) => drag(e, clip)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") select(clip.id);
                        }}
                      >
                        <Show
                          when={
                            !clip.motion?.title &&
                            workflow.offline.includes(clip.source.asset)
                          }
                        >
                          <span class="offline-label">Mídia Offline</span>
                        </Show>
                        <span
                          class="trim-edge head"
                          title="Aparar entrada · Shift: Ripple"
                          onPointerDown={(e) => drag(e, clip, "head")}
                        >
                          [|
                        </span>
                        <span
                          class="trim-edge tail"
                          title="Aparar saída · Shift: Ripple"
                          onPointerDown={(e) => drag(e, clip, "tail")}
                        >
                          |]
                        </span>
                        <For
                          each={Object.values(clip.motion?.keys ?? {})
                            .flat()
                            .filter(
                              (k) =>
                                seconds(k.at) >= seconds(clip.motion!.offset) &&
                                seconds(k.at) <
                                  seconds(clip.motion!.offset) +
                                    seconds(clip.duration),
                            )}
                        >
                          {(k) => (
                            <span
                              class="key-marker"
                              style={{
                                left: `${(100 * (seconds(k.at) - seconds(clip.motion!.offset))) / seconds(clip.duration)}%`,
                              }}
                            >
                              ◆
                            </span>
                          )}
                        </For>
                        <Show
                          when={
                            clip.motion &&
                            (seconds(clip.motion.fadeIn) > 0 ||
                              seconds(clip.motion.fadeOut) > 0 ||
                              seconds(clip.motion.dissolveIn) > 0)
                          }
                        >
                          <span class="transition-marker">◩ Transição</span>
                        </Show>
                        <Show
                          when={track.kind === "video" && !props.audio}
                          fallback={
                            <>
                              <Waveform clip={clip} />
                              <span class="audio-clip-label">{clip.name}</span>
                            </>
                          }
                        >
                          <div class="clip-thumb">
                            <span aria-label="Arquivo de vídeo">
                              {clip.motion?.title ? "T" : "▶"}
                            </span>
                          </div>
                          <span>{clip.name}</span>
                        </Show>
                      </div>
                    )}
                  </For>
                </div>
              )}
            </For>
            <Show when={guide() !== null}>
              <div
                class="snap-guide"
                style={{
                  left: `${(Number(guide()) / projectFrames()) * 100}%`,
                }}
              />
            </Show>
            <div
              class="playhead"
              onPointerDown={scrub}
              style={{
                left: `${(Number(editor.frame) / projectFrames()) * 100}%`,
              }}
            >
              <div />
            </div>
          </div>
        </div>
      </div>
      <div class="timeline-footer">
        <span>
          <Keyboard size={12} /> V Seleção · C Corte · N Encaixe · Ctrl+B
          Dividir · Ctrl+Z Desfazer
        </span>
        <span>
          {props.audio
            ? "Formas de onda reais · cortes e movimento preservam o áudio"
            : "Arraste um clipe para mover · cortes e movimentos no Rust"}
        </span>
      </div>
    </section>
  );
}
