import { For, Show } from "solid-js";
import { editor } from "../state/editor";
import {
  workflow,
  setWorkflow,
  generateProxy,
  relink,
  recover,
} from "../state/workflow";
export default function WorkflowDialogs() {
  return (
    <>
      <Show when={workflow.context} keyed>
        {(c) => (
          <div
            class="context-dismiss"
            onClick={() => setWorkflow("context", null)}
            onContextMenu={(e) => {
              e.preventDefault();
              setWorkflow("context", null);
            }}
          >
            <div
              role="menu"
              class="media-context"
              style={{ left: `${c.x}px`, top: `${c.y}px` }}
              onClick={(e) => e.stopPropagation()}
            >
              <button
                role="menuitem"
                disabled={
                  workflow.busy ||
                  workflow.offline.includes(c.id) ||
                  editor.snapshot.assets.find((a) => a.id === c.id)?.audio_only
                }
                onClick={() => void generateProxy(c.id)}
              >
                Gerar Proxy (720p)
              </button>
              <button
                role="menuitem"
                disabled={editor.busy}
                onClick={() => void relink(c.id)}
              >
                Reconectar Mídia…
              </button>
              <button
                role="menuitem"
                onClick={() => setWorkflow("context", null)}
              >
                Fechar
              </button>
            </div>
          </div>
        )}
      </Show>
      <Show when={workflow.recovery.length}>
        <div class="modal-backdrop">
          <section
            class="editorial-dialog"
            role="dialog"
            aria-modal="true"
            aria-label="Recuperar alterações"
          >
            <h2>Recuperar projeto</h2>
            <p>Deseja recuperar as alterações não salvas?</p>
            <p>
              Recuperações permanecem no disco até você escolher. Recuperar
              outra sessão substitui a timeline aberta.
            </p>
            <For each={workflow.recovery}>
              {(name) => (
                <div class="recovery-item">
                  <small>{name}</small>
                  <div class="dialog-actions">
                    <button
                      disabled={workflow.recovering}
                      onClick={() => void recover(name, false)}
                    >
                      Descartar
                    </button>
                    <button
                      disabled={workflow.recovering}
                      onClick={() => void recover(name, true)}
                    >
                      Recuperar
                    </button>
                  </div>
                </div>
              )}
            </For>
            <p role="alert">{editor.error}</p>
          </section>
        </div>
      </Show>
    </>
  );
}
