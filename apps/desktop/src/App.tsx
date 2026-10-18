import WorkflowDialogs from "./components/WorkflowDialogs";
import { connectWorkflow } from "./state/workflow";
import MarkerDialog, { addMarker } from "./components/MarkerDialog";
import { createTitle } from "./state/editor";
import { createSignal, onMount, onCleanup, Show } from "solid-js";
import {
  X,
  CheckCircle2,
  AlertCircle,
  PanelLeft,
  PanelRight,
} from "lucide-solid";
import Header from "./components/Header";
import Library from "./components/Library";
import Preview from "./components/Preview";
import Adjustments from "./components/Adjustments";
import Timeline from "./components/Timeline";
import NodeFlow from "./components/NodeFlow";
import DeliveryWorkspace from "./components/DeliveryWorkspace";
import { Equalizer, Mixer } from "./components/AudioWorkspace";
import {
  editor,
  setEditor,
  initialize,
  split,
  undo,
  rate,
  seek,
  togglePlayback,
  pausePlayback,
  saveProject,
  openProject,
} from "./state/editor";
export default function App() {
  const [left, setLeft] = createSignal(278),
    [right, setRight] = createSignal(292),
    [top, setTop] = createSignal(50);
  function resize(e: PointerEvent, kind: "left" | "right" | "top") {
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const origin = e.clientX,
      originY = e.clientY,
      base = kind === "left" ? left() : right(),
      baseTop = top();
    const update = (ev: PointerEvent) => {
      if (kind === "top")
        setTop(
          Math.min(
            66,
            Math.max(
              42,
              baseTop + ((ev.clientY - originY) / window.innerHeight) * 100,
            ),
          ),
        );
      else
        (kind === "left" ? setLeft : setRight)(
          Math.min(
            390,
            Math.max(
              230,
              base + (ev.clientX - origin) * (kind === "left" ? 1 : -1),
            ),
          ),
        );
    };
    const stop = () => {
      el.removeEventListener("pointermove", update);
      el.removeEventListener("pointerup", stop);
      el.removeEventListener("pointercancel", stop);
    };
    el.addEventListener("pointermove", update);
    el.addEventListener("pointerup", stop);
    el.addEventListener("pointercancel", stop);
  }
  onMount(() => {
    let disposed = false,
      cleanup: () => void = () => {};
    void initialize().then(async (fn) => {
      if (disposed) {
        fn();
        return;
      }
      cleanup = fn;
      if (editor.connected) {
        const workflowStop = await connectWorkflow();
        if (disposed) workflowStop();
        else
          cleanup = () => {
            fn();
            workflowStop();
          };
      }
    });
    const keyboard = (e: KeyboardEvent) => {
      if (document.querySelector('[aria-modal="true"]')) return;
      const key = e.key.toLowerCase();
      if (
        (e.target as HTMLElement).closest(
          'input,textarea,select,[contenteditable="true"]',
        )
      )
        return;
      if ((e.ctrlKey || e.metaKey) && key === "s") {
        e.preventDefault();
        void saveProject();
      } else if ((e.ctrlKey || e.metaKey) && key === "o") {
        e.preventDefault();
        void openProject();
      } else if ((e.ctrlKey || e.metaKey) && key === "z") {
        e.preventDefault();
        void undo();
      } else if ((e.ctrlKey || e.metaKey) && key === "b") {
        e.preventDefault();
        void split();
      } else if (key === " ") {
        e.preventDefault();
        togglePlayback();
      } else if (key === "v") setEditor("tool", "select");
      else if (key === "t") void createTitle();
      else if (key === "c") setEditor("tool", "cut");
      else if (key === "m") addMarker();
      else if (key === "n") setEditor("snap", !editor.snap);
      else if (key === "i") {
        setEditor("inFrame", editor.frame);
        if (editor.outFrame < editor.frame) setEditor("outFrame", editor.frame);
      } else if (key === "o") {
        setEditor("outFrame", editor.frame);
        if (editor.inFrame > editor.frame) setEditor("inFrame", editor.frame);
      } else if (key === "arrowleft") {
        e.preventDefault();
        seek(editor.frame - 1n);
      } else if (key === "arrowright") {
        e.preventDefault();
        seek(editor.frame + 1n);
      }
    };
    window.addEventListener("keydown", keyboard);
    const pause = () => {
      if (document.hidden && editor.playing) pausePlayback();
    };
    document.addEventListener("visibilitychange", pause);
    onCleanup(() => {
      disposed = true;
      cleanup();
      window.removeEventListener("keydown", keyboard);
      document.removeEventListener("visibilitychange", pause);
    });
  });
  return (
    <div class="app-shell">
      <Header />
      <MarkerDialog />
      <WorkflowDialogs />
      <Show when={editor.space !== "entrega"} fallback={<DeliveryWorkspace />}>
        <main class="workspace" style={{ "--top-size": `${top()}%` }}>
          <div
            class="upper-workspace"
            style={{
              "grid-template-columns": `${left()}px 7px minmax(300px,1fr) 7px ${right()}px`,
            }}
          >
            <Library />
            <div
              class="resize-handle vertical"
              role="separator"
              aria-label="Largura do Acervo"
              onPointerDown={(e) => resize(e, "left")}
            />
            <Show when={editor.space === "som"} fallback={<Preview />}>
              <Equalizer />
            </Show>
            <div
              class="resize-handle vertical"
              role="separator"
              aria-label="Largura dos Ajustes"
              onPointerDown={(e) => resize(e, "right")}
            />
            <Show when={editor.space === "som"} fallback={<Adjustments />}>
              <Mixer />
            </Show>
          </div>
          <div
            class="resize-handle horizontal"
            role="separator"
            aria-label="Altura dos painéis"
            onPointerDown={(e) => resize(e, "top")}
          />
          <Show
            when={editor.space === "trama"}
            fallback={<Timeline audio={editor.space === "som"} />}
          >
            <NodeFlow />
          </Show>
        </main>
      </Show>
      <footer class="status-bar">
        <span>
          <span class="status-dot" classList={{ muted: !editor.connected }} />
          {editor.loading
            ? "Conectando ao motor…"
            : editor.connected
              ? "Motor Rust conectado"
              : "Prévia web · somente leitura"}
          <span class="status-separator" />
          Rev. {editor.snapshot.revision}
          <span class="status-separator" />
          {editor.busy ? "Aplicando edição…" : "Composição ativa"}
        </span>
        <span>
          Áudio nativo · composição e efeitos ativos
          <span class="status-separator" />
          <button
            title="Restaurar painéis"
            onClick={() => {
              setLeft(278);
              setRight(292);
              setTop(50);
            }}
          >
            <PanelLeft size={13} />
            <PanelRight size={13} />
          </button>
        </span>
      </footer>
      <Show when={editor.error || editor.notice}>
        <div
          class="toast"
          role={editor.error ? "alert" : "status"}
          classList={{ error: !!editor.error }}
        >
          {editor.error ? (
            <AlertCircle size={19} />
          ) : (
            <CheckCircle2 size={19} />
          )}
          <span>{editor.error || editor.notice}</span>
          <button
            aria-label="Fechar aviso"
            onClick={() => setEditor({ error: "", notice: "" })}
          >
            <X size={16} />
          </button>
        </div>
      </Show>
    </div>
  );
}
