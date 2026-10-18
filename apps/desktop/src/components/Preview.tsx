import { onMount, onCleanup, createEffect, createSignal, Show } from "solid-js";
import {
  Play,
  Pause,
  SkipBack,
  SkipForward,
  ChevronLeft,
  ChevronRight,
  Maximize,
} from "lucide-solid";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  editor,
  setEditor,
  rate,
  seek,
  report,
  togglePlayback,
} from "../state/editor";
import { timecode } from "../lib/time";
import { viewport, native } from "../lib/ipc";
import { presenter } from "../lib/presenter";
export default function Preview() {
  let surface!: HTMLDivElement, canvas!: HTMLCanvasElement;
  let renderer: ReturnType<typeof presenter> | undefined;
  let alive = true,
    geometryFlight = false,
    geometryDirty = false,
    raf = 0;
  let fetching = false,
    wanted = false,
    lastKey = "",
    abort: AbortController | undefined;
  const [size, setSize] = createSignal(""),
    [shown, setShown] = createSignal(false),
    [displayError, setDisplayError] = createSignal("");
  async function geometry() {
    geometryDirty = true;
    if (geometryFlight) return;
    geometryFlight = true;
    try {
      while (geometryDirty && alive) {
        geometryDirty = false;
        const r = surface.getBoundingClientRect();
        setSize(
          `${Math.round(r.width * devicePixelRatio)} × ${Math.round(r.height * devicePixelRatio)}`,
        );
        renderer?.draw();
        await viewport({
          x: r.x,
          y: r.y,
          width: r.width,
          height: r.height,
          devicePixelRatio,
          visible:
            editor.space !== "som" &&
            !document.hidden &&
            r.width > 0 &&
            r.height > 0,
        });
      }
    } catch (e) {
      report(e);
    } finally {
      geometryFlight = false;
    }
  }
  const schedule = () => {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => void geometry());
  };
  async function fetchFrame() {
    wanted = true;
    if (fetching || !renderer || !native) return;
    fetching = true;
    try {
      while (wanted && alive) {
        wanted = false;
        const t = editor.transport;
        const generation = t.generation,
          key = `${generation}:${t.presentedSequence}`;
        if (editor.transportPending || !t.presentedSequence || key === lastKey)
          continue;
        abort = new AbortController();
        const response = await fetch(
          `${convertFileSrc("", "meraki-frame")}frame/${generation}`,
          { cache: "no-store", signal: abort.signal },
        );
        if (response.status === 204) continue;
        if (!response.ok)
          throw new Error(`Prévia binária: HTTP ${response.status}`);
        const bytes = await response.arrayBuffer();
        if (
          !alive ||
          editor.transportPending ||
          editor.transport.generation !== generation ||
          response.headers.get("X-Generation") !== generation
        )
          continue;
        lastKey = `${generation}:${response.headers.get("X-Sequence")}`;
        if (response.headers.get("X-Blank") === "1") {
          renderer.clear();
          setShown(false);
        } else {
          const w = Number(response.headers.get("X-Width")),
            h = Number(response.headers.get("X-Height"));
          if (Number(response.headers.get("X-Stride")) !== w * 4)
            throw new Error("Stride de preview incompatível");
          renderer.upload(bytes, w, h);
          setShown(true);
          setDisplayError("");
        }
      }
    } catch (e) {
      if (!(e instanceof DOMException && e.name === "AbortError"))
        setDisplayError(e instanceof Error ? e.message : String(e));
    } finally {
      fetching = false;
      if (wanted && alive) void fetchFrame();
    }
  }
  onMount(() => {
    try {
      renderer = presenter(canvas);
    } catch (e) {
      setDisplayError(String(e));
    }
    const observer = new ResizeObserver(schedule);
    observer.observe(surface);
    window.addEventListener("resize", schedule);
    window.addEventListener("scroll", schedule, true);
    document.addEventListener("visibilitychange", schedule);
    let dpi = matchMedia(`(resolution: ${devicePixelRatio}dppx)`);
    const dpiChange = () => {
      dpi.removeEventListener("change", dpiChange);
      dpi = matchMedia(`(resolution: ${devicePixelRatio}dppx)`);
      dpi.addEventListener("change", dpiChange);
      schedule();
    };
    dpi.addEventListener("change", dpiChange);
    const lost = (e: Event) => {
      e.preventDefault();
      setDisplayError(
        "Contexto GPU perdido. Alterne para Som e volte para recriar a Prévia.",
      );
    };
    canvas.addEventListener("webglcontextlost", lost);
    schedule();
    void fetchFrame();
    onCleanup(() => {
      alive = false;
      abort?.abort();
      observer.disconnect();
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", schedule);
      window.removeEventListener("scroll", schedule, true);
      document.removeEventListener("visibilitychange", schedule);
      dpi.removeEventListener("change", dpiChange);
      canvas.removeEventListener("webglcontextlost", lost);
      renderer?.dispose();
      void viewport({
        x: 0,
        y: 0,
        width: 0,
        height: 0,
        devicePixelRatio,
        visible: false,
      }).catch(() => {});
    });
  });
  let displayedGeneration = "";
  createEffect(() => {
    const generation = editor.transport.generation;
    editor.transportPending;
    if (editor.transportPending || displayedGeneration !== generation) {
      abort?.abort();
      // Keep the last completed texture during a visual edit to avoid black flashes.
      // Explicit transport actions still clear it; generation checks reject stale fetches.
      if (editor.transportPending || !editor.snapshot.timeline.tracks.some(t=>t.clips.length)) {
        renderer?.clear();
        setShown(false);
      }
      lastKey = "";
    }
    displayedGeneration = generation;
    void fetchFrame();
  });
  createEffect(() => {
    editor.transport.presentedSequence;
    void fetchFrame();
  });
  return (
    <section class="panel preview-panel">
      <div class="panel-title">
        <h2>
          Prévia <span class="quiet">/ Sequência principal</span>
        </h2>
        <div class="inline">
          <span class="small-label" title={editor.transport.compositor}>1080p · {editor.transport.compositor?.startsWith("CPU") ? "CPU" : editor.transport.compositor ? "GPU" : "Inicializando"}</span>
          <button
            class="icon-button"
            title="Tela cheia da prévia"
            onClick={() => {
              if (document.fullscreenElement)
                void document.exitFullscreen().catch(report);
              else void surface.requestFullscreen().catch(report);
            }}
          >
            <Maximize size={14} />
          </button>
        </div>
      </div>
      <div class="viewport-wrap">
        <div
          class="native-viewport real-viewport"
          ref={surface}
          data-native-viewport
          aria-label="Prévia de vídeo real"
        >
          <canvas
            ref={canvas}
            class="video-canvas"
            aria-label="Frames decodificados do arquivo"
          />
          <Show when={!shown() || editor.transport.error || displayError()}>
            <div class="preview-empty">
              <strong>
                {displayError() ||
                  editor.transport.error ||
                  (editor.transport.buffering
                    ? "Buscando quadro…"
                    : editor.snapshot.assets.length
                      ? "Sem vídeo nesta posição"
                      : "Importe um vídeo para começar")}
              </strong>
              <small>
                {editor.snapshot.assets.length
                  ? "Fonte local · decodificação FFmpeg"
                  : "MP4 · MOV · MKV · WebM"}
              </small>
            </div>
          </Show>
        </div>
      </div>
      <div class="preview-position">
        <div
          style={{
            width: `${editor.outFrame ? Math.min(100, (Number(editor.frame) * 100) / Number(editor.outFrame)) : 0}%`,
          }}
        />
      </div>
      <div class="transport">
        <div class="in-out">
          <button
            title="Marcar entrada (I)"
            onClick={() => setEditor("inFrame", editor.frame)}
          >
            I
          </button>
          <button
            title="Marcar saída (O)"
            onClick={() => setEditor("outFrame", editor.frame)}
          >
            O
          </button>
        </div>
        <div class="transport-center">
          <button title="Ir à entrada" onClick={() => seek(editor.inFrame)}>
            <SkipBack size={16} />
          </button>
          <button
            title="Quadro anterior"
            onClick={() => seek(editor.frame - 1n)}
          >
            <ChevronLeft size={17} />
          </button>
          <button
            class="play"
            title="Reproduzir/pausar (Espaço)"
            aria-label={editor.playing ? "Pausar" : "Reproduzir"}
            disabled={!editor.connected || !editor.snapshot.assets.length}
            onClick={togglePlayback}
          >
            {editor.playing ? (
              <Pause size={20} fill="currentColor" />
            ) : (
              <Play size={20} fill="currentColor" />
            )}
          </button>
          <button
            title="Próximo quadro"
            onClick={() => seek(editor.frame + 1n)}
          >
            <ChevronRight size={17} />
          </button>
          <button title="Ir à saída" onClick={() => seek(editor.outFrame)}>
            <SkipForward size={16} />
          </button>
        </div>
        <output class="timecode">{timecode(editor.frame, rate())}</output>
      </div>
      <div class="preview-meta">
        <span>
          {size()} px · cache{" "}
          {(editor.transport.cacheBytes / 1048576).toFixed(1)} MiB
        </span>
        <span>Compor {editor.transport.decodeMs.toFixed(1)} ms · áudio nativo</span>
      </div>
    </section>
  );
}
