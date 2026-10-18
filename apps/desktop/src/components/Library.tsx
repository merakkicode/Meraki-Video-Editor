import { workflow, mediaContext } from "../state/workflow";
import { setVisualNode } from "../state/visual";
import { createSignal, For, Show } from "solid-js";
import { Search, Plus, Film, Circle, SlidersHorizontal } from "lucide-solid";
import { editor, importMedia, insertMedia, canEdit } from "../state/editor";
import { seconds } from "../lib/time";
export function Waveform(_props: { seed?: number }) {
  return (
    <svg
      class="waveform"
      viewBox="0 0 600 60"
      aria-label="Forma de onda não disponível"
    >
      <path d="M0 30H600" stroke="currentColor" />
    </svg>
  );
}
export default function Library() {
  const [search, setSearch] = createSignal("");
  const effects = [
    { name: "Desfoque", icon: Circle },
    { name: "Ajuste de Cor", icon: SlidersHorizontal },
  ];
  return (
    <section class="panel library-panel">
      <div class="panel-title">
        <h2>{editor.space === "trama" ? "Efeitos" : "Acervo"}</h2>
        <button
          class="icon-button"
          title="Importar mídia"
          aria-label="Importar mídia"
          disabled={!canEdit()}
          onClick={() => void importMedia()}
        >
          <Plus size={18} />
        </button>
      </div>
      <label class="search-box">
        <Search size={15} />
        <input
          placeholder="Buscar no acervo…"
          value={search()}
          onInput={(e) => setSearch(e.currentTarget.value)}
        />
      </label>
      <Show
        when={editor.space !== "trama"}
        fallback={
          <>
            <p class="media-help">
              Selecione um clipe de vídeo e ajuste os efeitos no painel à
              direita.
            </p>
            <div class="effect-grid">
              <For
                each={effects.filter((e) =>
                  e.name.toLowerCase().includes(search().toLowerCase()),
                )}
              >
                {(e) => (
                  <button
                    class="effect-card"
                    onClick={() => setVisualNode(e.name === "Desfoque" ? 3 : 2)}
                  >
                    <e.icon size={32} />
                    <span>{e.name}</span>
                  </button>
                )}
              </For>
            </div>
          </>
        }
      >
        <div class="media-actions">
          <button
            class="import-button"
            disabled={!canEdit()}
            onClick={() => void importMedia()}
          >
            <Plus size={16} /> {editor.busy ? "Processando…" : "Importar mídia"}
          </button>
        </div>
        <p class="media-help">
          Vídeo entra em V1; WAV, MP3 e outras fontes de áudio entram em A1. Use
          “Inserir” para reutilizá-lo.
        </p>
        <div class="real-asset-list">
          <For
            each={editor.snapshot.assets.filter((a) =>
              a.name.toLowerCase().includes(search().toLowerCase()),
            )}
          >
            {(asset) => (
              <article
                class="real-asset"
                classList={{ offline: workflow.offline.includes(asset.id) }}
                onContextMenu={(e) => mediaContext(e, asset.id)}
              >
                <Show when={workflow.offline.includes(asset.id)}>
                  <span class="offline-label">Mídia Offline</span>
                </Show>
                <Show when={workflow.proxies.includes(asset.id)}>
                  <small>ϟ Proxy 720p pronto</small>
                </Show>
                <div class="real-asset-icon">
                  <Film size={26} />
                  <span>
                    {asset.audio_only
                      ? "Áudio"
                      : `${asset.width} × ${asset.height}`}
                  </span>
                </div>
                <strong title={asset.path}>{asset.name}</strong>
                <small>
                  {seconds(asset.duration).toFixed(2)} s · {asset.codec}
                  {!asset.audio_only &&
                    ` · ${(asset.frame_rate.numerator / asset.frame_rate.denominator).toFixed(2)} fps`}
                </small>
                <button
                  disabled={!canEdit()}
                  onClick={() => void insertMedia(asset.id)}
                >
                  Inserir na timeline
                </button>
                <For
                  each={editor.snapshot.timeline.tracks.filter(
                    (t) => t.kind === (asset.audio_only ? "audio" : "video"),
                  )}
                >
                  {(track) => (
                    <button
                      disabled={!canEdit()}
                      onClick={() => void insertMedia(asset.id, track.id)}
                    >
                      Inserir em {track.name}
                    </button>
                  )}
                </For>
              </article>
            )}
          </For>
          <Show when={!editor.snapshot.assets.length}>
            <div class="empty-media">
              <Film size={32} />
              <p>Seu filme começa aqui.</p>
              <small>
                Selecione um arquivo local para ver os frames reais.
              </small>
            </div>
          </Show>
        </div>
      </Show>
      <div class="library-footer" title={workflow.message}>
        {workflow.message}
        <br />
        {editor.snapshot.assets.length} arquivo(s) · referências locais
      </div>
    </section>
  );
}
