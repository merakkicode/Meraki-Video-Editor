import { workflow, toggleProxies } from "../state/workflow";
import { For, Show } from "solid-js";
import {
  Film,
  Workflow,
  AudioLines,
  Download,
  Minus,
  Square,
  X,
  ChevronDown,
} from "lucide-solid";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { native } from "../lib/ipc";
import {
  editor,
  setEditor,
  report,
  saveProject,
  openProject,
  type Space,
} from "../state/editor";
export default function Header() {
  const windowAction = async (
    action: "minimize" | "toggleMaximize" | "close",
  ) => {
    if (native)
      try {
        await getCurrentWindow()[action]();
      } catch (e) {
        report(e);
      }
  };
  const tabs = [
    { id: "montagem", label: "Montagem", icon: Film },
    { id: "trama", label: "Trama", icon: Workflow },
    { id: "som", label: "Som", icon: AudioLines },
    { id: "entrega", label: "Entrega", icon: Download },
  ] as const;
  return (
    <>
      <header class="app-header">
        <div class="brand" data-tauri-drag-region>
          <span class="brand-mark">〈M〉</span>
          <strong>MERAKI</strong>
          <span class="brand-divider" />
          <span class="brand-sub">Meraki Video Editor</span>
        </div>
        <nav class="space-tabs" aria-label="Espaços criativos">
          <For each={tabs}>
            {(tab) => (
              <button
                classList={{ active: editor.space === tab.id }}
                onClick={() => setEditor("space", tab.id as Space)}
                aria-pressed={editor.space === tab.id}
              >
                <tab.icon size={17} />
                {tab.label}
              </button>
            )}
          </For>
        </nav>
        <div class="header-actions">
          <button
            class="project-action"
            classList={{ active: workflow.enabled }}
            title="Somente na Prévia; Entrega usa originais"
            disabled={!editor.connected}
            aria-pressed={workflow.enabled}
            onClick={() => void toggleProxies()}
          >
            ϟ Usar Proxies
          </button>
          <button
            class="project-action"
            disabled={editor.busy || !native}
            onClick={() => void openProject()}
            title="Abrir projeto (Ctrl+O)"
          >
            Abrir
          </button>
          <button
            class="project-action"
            disabled={editor.busy || !native}
            onClick={() => void saveProject()}
            title="Salvar projeto (Ctrl+S)"
          >
            Salvar
          </button>
          <button class="export" onClick={() => setEditor("space", "entrega")}>
            <Download size={16} />
            Exportar
          </button>
          <div class="window-controls">
            <button
              title="Minimizar"
              aria-label="Minimizar"
              disabled={!native}
              onClick={() => windowAction("minimize")}
            >
              <Minus size={15} />
            </button>
            <button
              title="Maximizar ou restaurar"
              aria-label="Maximizar ou restaurar"
              disabled={!native}
              onClick={() => windowAction("toggleMaximize")}
            >
              <Square size={12} />
            </button>
            <button
              class="close"
              title="Fechar"
              aria-label="Fechar"
              disabled={!native}
              onClick={() => windowAction("close")}
            >
              <X size={17} />
            </button>
          </div>
        </div>
      </header>
      <div class="project-bar">
        <div class="project-name">
          <span class="status-dot" />
          {editor.snapshot.timeline.name}
          <ChevronDown size={13} />
          <span class="project-tag">v1.0.0</span>
          <small title={workflow.autosave}>{workflow.autosave}</small>
        </div>
        <span>
          <Show
            when={editor.space === "trama"}
            fallback={`${editor.snapshot.timeline.frameRate.numerator}/${editor.snapshot.timeline.frameRate.denominator} fps · Preview de vídeo`}
          >
            Efeitos por nós
          </Show>
        </span>
      </div>
    </>
  );
}
