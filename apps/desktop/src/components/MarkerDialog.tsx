import { For, Show, createSignal } from "solid-js";
import type { MarkerDto } from "../../../../contracts/ipc";
import { edit, editor, rate } from "../state/editor";
import { frameTime, newId } from "../lib/time";
export const [editingMarker, setEditingMarker] = createSignal<MarkerDto | null>(
  null,
);
export function addMarker() {
  void edit({
    kind: "marker",
    remove: false,
    marker: {
      id: newId(),
      at: frameTime(editor.frame, rate()),
      name: "Marcador",
      color: "cyan",
    },
  });
}
export default function MarkerDialog() {
  return (
    <Show when={editingMarker()} keyed>
      {(m) => {
        const [name, setName] = createSignal(m.name),
          [color, setColor] = createSignal(m.color);
        const commit = async (remove = false) => {
          await edit({
            kind: "marker",
            marker: { ...m, name: name(), color: color() },
            remove,
          });
          if (!editor.error) setEditingMarker(null);
        };
        return (
          <div class="modal-backdrop">
            <form
              class="editorial-dialog"
              role="dialog"
              aria-modal="true"
              aria-label="Editar marcador"
              onSubmit={(e) => {
                e.preventDefault();
                void commit();
              }}
            >
              <h2>Marcador</h2>
              <label>
                Anotação
                <input
                  autofocus
                  maxlength="1024"
                  value={name()}
                  onInput={(e) => setName(e.currentTarget.value)}
                />
              </label>
              <label>
                Cor
                <select
                  value={color()}
                  onChange={(e) =>
                    setColor(e.currentTarget.value as MarkerDto["color"])
                  }
                >
                  <For
                    each={[
                      ["cyan", "Ciano"],
                      ["violet", "Violeta"],
                      ["yellow", "Amarelo"],
                      ["red", "Vermelho"],
                    ]}
                  >
                    {(c) => <option value={c[0]}>{c[1]}</option>}
                  </For>
                </select>
              </label>
              <p role="alert">{editor.error}</p>
              <div class="dialog-actions">
                <button type="button" onClick={() => setEditingMarker(null)}>
                  Cancelar
                </button>
                <button type="button" onClick={() => void commit(true)}>
                  Excluir
                </button>
                <button type="submit" disabled={editor.busy}>
                  Salvar
                </button>
              </div>
            </form>
          </div>
        );
      }}
    </Show>
  );
}
