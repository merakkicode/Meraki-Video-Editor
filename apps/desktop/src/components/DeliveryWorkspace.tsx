import { flushVisual } from "../state/visual";
import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { createStore } from "solid-js/store";
import {
  Download,
  FolderOpen,
  Plus,
  Play,
  X,
  CheckCircle2,
  Film,
  Smartphone,
  Globe,
  Archive,
  SlidersHorizontal,
} from "lucide-solid";
import type { ExportSettingsDto } from "../../../../contracts/ipc";
import Preview from "./Preview";
import Timeline from "./Timeline";
import {
  editor,
  rate,
  report,
  setEditor,
  pausePlayback,
} from "../state/editor";
import {
  delivery,
  connectDelivery,
  addJob,
  startQueue,
  cancelJob,
  browseFolder,
} from "../state/delivery";
const statusName = {
  ready: "Pronto",
  rendering: "Renderizando",
  completed: "Concluído",
  cancelled: "Cancelado",
  error: "Erro",
};
const icons = [Film, Smartphone, Globe, Archive, SlidersHorizontal];
export default function DeliveryWorkspace() {
  const [settings, setSettings] = createStore<ExportSettingsDto>({
    preset: "youtube",
    output: "",
    container: "mp4",
    width: 1920,
    height: 1080,
    frameRate: rate(),
    codec: "libx264",
    rateMode: "crf",
    crf: 18,
    bitrateMbps: 20,
    audioCodec: "aac",
    audioBitrateKbps: 320,
    sampleRate: 48000,
    channels: 2,
  });
  const [folder, setFolder] = createSignal(""),
    [filename, setFilename] = createSignal("Meu filme"),
    [rangeMode, setRangeMode] = createSignal("all"),
    [busy, setBusy] = createSignal(false);
  const [start, setStart] = createSignal(editor.inFrame.toString()),
    [end, setEnd] = createSignal(editor.outFrame.toString());
  const fullEnd = () =>
    editor.snapshot.timeline.tracks
      .flatMap((t) => t.clips)
      .reduce((max, c) => {
        const n =
          BigInt(c.timelineStart.numerator) * BigInt(c.duration.denominator) +
          BigInt(c.duration.numerator) * BigInt(c.timelineStart.denominator);
        const d =
          BigInt(c.timelineStart.denominator) * BigInt(c.duration.denominator);
        const a = n * BigInt(rate().numerator),
          b = d * BigInt(rate().denominator);
        const value = (a + b - 1n) / b;
        return value > max ? value : max;
      }, 0n);
  const output = () =>
    `${folder().replace(/[\\/]$/, "")}/${filename()}.${settings.container}`;
  onMount(() => {
    let disposed = false,
      stop = () => {};
    void connectDelivery()
      .then((fn) => {
        if (disposed) fn();
        else stop = fn;
      })
      .catch(report);
    onCleanup(() => {
      disposed = true;
      stop();
    });
  });
  async function browse() {
    try {
      const path = await browseFolder();
      if (path) setFolder(path);
    } catch (e) {
      report(e);
    }
  }
  async function enqueue() {
    if (busy()) return;
    if (
      !folder() ||
      !filename().trim() ||
      /[<>:"/\\|?*]/.test(filename()) ||
      /[. ]$/.test(filename())
    ) {
      report("Escolha a pasta e um nome de arquivo válido, sem extensão.");
      return;
    }
    setBusy(true);
    try {
      await flushVisual();
      await addJob(
        { ...settings, output: output() },
        {
          startFrame: rangeMode() === "all" ? "0" : start(),
          endFrame: rangeMode() === "all" ? fullEnd().toString() : end(),
        },
      );
      setEditor(
        "notice",
        "Trabalho adicionado com um snapshot independente da timeline.",
      );
    } catch (e) {
      report(e);
    } finally {
      setBusy(false);
    }
  }
  const custom = () => setSettings("preset", "custom");
  const marker = (value: string) =>
    Math.max(
      0,
      Math.min(
        100,
        ((Number(value) || 0) * 100) / Math.max(1, Number(fullEnd())),
      ),
    );
  return (
    <main class="delivery-workspace">
      <aside class="panel delivery-settings">
        <div class="panel-title">
          <h2>
            <Download size={16} /> Entrega
          </h2>
          <span class="small-label">RENDER OFFLINE</span>
        </div>
        <div class="delivery-scroll">
          <div class="delivery-presets">
            <For each={delivery.presets}>
              {(preset, i) => {
                const Icon = icons[i()];
                return (
                  <button
                    class="delivery-preset"
                    classList={{ active: settings.preset === preset.id }}
                    onClick={() =>
                      setSettings({ ...preset.settings, output: "" })
                    }
                    title={preset.description}
                  >
                    <Icon size={19} />
                    <strong>{preset.name}</strong>
                    <small>{preset.description}</small>
                  </button>
                );
              }}
            </For>
          </div>
          <h3>Destino</h3>
          <label>
            Nome do arquivo
            <div class="output-name">
              <input
                aria-label="Nome do arquivo de exportação"
                value={filename()}
                onInput={(e) => setFilename(e.currentTarget.value)}
              />
              <span>.{settings.container}</span>
            </div>
          </label>
          <label>
            Pasta de saída
            <div class="folder-picker">
              <input
                aria-label="Pasta de saída"
                value={folder()}
                readOnly
                placeholder="Selecione uma pasta…"
              />
              <button
                title="Selecionar pasta de saída"
                onClick={() => void browse()}
              >
                <FolderOpen size={17} />
              </button>
            </div>
          </label>
          <h3>Vídeo</h3>
          <div class="delivery-fields">
            <label>
              Contêiner
              <select
                value={settings.container}
                onChange={(e) => {
                  custom();
                  setSettings(
                    "container",
                    e.currentTarget.value as ExportSettingsDto["container"],
                  );
                }}
              >
                <option value="mp4">MP4</option>
                <option value="mov">MOV</option>
                <option value="mkv">MKV</option>
              </select>
            </label>
            <label>
              Resolução
              <select
                value={`${settings.width}x${settings.height}`}
                onChange={(e) => {
                  const [w, h] = e.currentTarget.value.split("x").map(Number);
                  if (w && h) {
                    setSettings({ width: w, height: h });
                  }
                }}
              >
                <option value="1920x1080">1080p · 1920×1080</option>
                <option value="2560x1440">1440p · 2560×1440</option>
                <option value="3840x2160">4K UHD · 3840×2160</option>
                <option value="1080x1920">Vertical · 1080×1920</option>
                <option value={`${settings.width}x${settings.height}`}>
                  Personalizada
                </option>
              </select>
            </label>
            <label>
              Largura
              <input
                type="number"
                min="64"
                max="4096"
                step="2"
                value={settings.width}
                onInput={(e) => {
                  custom();
                  setSettings("width", e.currentTarget.valueAsNumber);
                }}
              />
            </label>
            <label>
              Altura
              <input
                type="number"
                min="64"
                max="4096"
                step="2"
                value={settings.height}
                onInput={(e) => {
                  custom();
                  setSettings("height", e.currentTarget.valueAsNumber);
                }}
              />
            </label>
            <label>
              FPS · numerador
              <input
                type="number"
                min="1"
                value={settings.frameRate.numerator}
                onInput={(e) => {
                  custom();
                  setSettings(
                    "frameRate",
                    "numerator",
                    e.currentTarget.valueAsNumber,
                  );
                }}
              />
            </label>
            <label>
              FPS · denominador
              <input
                type="number"
                min="1"
                value={settings.frameRate.denominator}
                onInput={(e) => {
                  custom();
                  setSettings(
                    "frameRate",
                    "denominator",
                    e.currentTarget.valueAsNumber,
                  );
                }}
              />
            </label>
            <label>
              Codec
              <select
                value={settings.codec}
                onChange={(e) => {
                  custom();
                  const codec = e.currentTarget
                    .value as ExportSettingsDto["codec"];
                  setSettings("codec", codec);
                  if (codec === "prores_ks")
                    setSettings({
                      container: "mov",
                      rateMode: "quality",
                      audioCodec: "pcm_s24le",
                    });
                  else setSettings("rateMode", "crf");
                }}
              >
                <option value="libx264">H.264 · libx264</option>
                <option value="libx265">H.265 · libx265</option>
                <option value="prores_ks">ProRes 422 HQ</option>
              </select>
            </label>
            <label>
              Modo de taxa
              <select
                value={settings.rateMode}
                disabled={settings.codec === "prores_ks"}
                onChange={(e) => {
                  custom();
                  setSettings(
                    "rateMode",
                    e.currentTarget.value as ExportSettingsDto["rateMode"],
                  );
                }}
              >
                <option value="crf">Qualidade · CRF</option>
                <option value="bitrate">Bitrate alvo</option>
                <Show when={settings.codec === "prores_ks"}>
                  <option value="quality">ProRes · HQ</option>
                </Show>
              </select>
            </label>
            <Show when={settings.codec !== "prores_ks"}>
              <label>
                {settings.rateMode === "crf"
                  ? "CRF · menor = melhor"
                  : "Bitrate · Mbps"}
                <input
                  type="number"
                  min={settings.rateMode === "crf" ? "0" : "1"}
                  max={settings.rateMode === "crf" ? "51" : "500"}
                  value={
                    settings.rateMode === "crf"
                      ? settings.crf
                      : settings.bitrateMbps
                  }
                  onInput={(e) => {
                    custom();
                    setSettings(
                      settings.rateMode === "crf" ? "crf" : "bitrateMbps",
                      e.currentTarget.valueAsNumber,
                    );
                  }}
                />
              </label>
            </Show>
          </div>
          <h3>Áudio da mídia</h3>
          <div class="delivery-fields">
            <label>
              Codec
              <select
                value={settings.audioCodec}
                onChange={(e) => {
                  custom();
                  setSettings(
                    "audioCodec",
                    e.currentTarget.value as ExportSettingsDto["audioCodec"],
                  );
                }}
              >
                <option value="aac">AAC</option>
                <option value="pcm_s24le">PCM 24-bit</option>
                <option value="none">Sem áudio</option>
              </select>
            </label>
            <label>
              Bitrate AAC · kbps
              <input
                type="number"
                min="64"
                max="320"
                step="32"
                disabled={settings.audioCodec !== "aac"}
                value={settings.audioBitrateKbps}
                onInput={(e) => {
                  custom();
                  setSettings(
                    "audioBitrateKbps",
                    e.currentTarget.valueAsNumber,
                  );
                }}
              />
            </label>
          </div>
          <p class="delivery-hint">
            48 kHz · Estéreo. Áudio embutido acompanha os clipes; gaps e fontes
            sem áudio ficam em silêncio. A Mesa e o EQ são incluídos na
            exportação.
          </p>
          <p class="delivery-hint">
            Canvas 1920 × 1080 proporcional com barras pretas. Composição com
            alfa; transformações, cor e desfoque são incluídos. ProRes reduz
            perdas de codificação, mas não recupera precisão acima de 8 bits.
          </p>
        </div>
        <button
          class="delivery-add"
          disabled={busy() || !editor.connected || fullEnd() === 0n}
          onClick={() => void enqueue()}
        >
          <Plus size={17} />
          {busy() ? "Adicionando…" : "Adicionar à Fila de Render"}
        </button>
      </aside>
      <section class="delivery-center">
        <Preview />
        <section class="panel delivery-range">
          <div
            class="render-range-bar"
            aria-label="Intervalo selecionado na sequência"
          >
            <span
              style={{
                left: rangeMode() === "all" ? "0%" : `${marker(start())}%`,
                right: rangeMode() === "all" ? "0%" : `${100 - marker(end())}%`,
              }}
            />
          </div>
          <div>
            <strong>Intervalo de render</strong>
            <small>Out exclusivo · índices de frames do projeto</small>
          </div>
          <select
            aria-label="Intervalo de render"
            value={rangeMode()}
            onChange={(e) => setRangeMode(e.currentTarget.value)}
          >
            <option value="all">
              Toda a Linha do Tempo · {fullEnd().toString()} frames
            </option>
            <option value="marks">Pontos In / Out</option>
          </select>
          <Show when={rangeMode() === "marks"}>
            <label>
              In
              <input
                aria-label="Frame In"
                inputmode="numeric"
                value={start()}
                onInput={(e) => setStart(e.currentTarget.value)}
              />
            </label>
            <label>
              Out
              <input
                aria-label="Frame Out exclusivo"
                inputmode="numeric"
                value={end()}
                onInput={(e) => setEnd(e.currentTarget.value)}
              />
            </label>
            <button
              onClick={() => {
                setStart(editor.inFrame.toString());
                setEnd(editor.outFrame.toString());
              }}
            >
              Usar marcas I/O
            </button>
          </Show>
        </section>
        <div class="delivery-timeline">
          <Timeline />
        </div>
      </section>
      <aside class="panel delivery-queue">
        <div class="panel-title">
          <h2>Fila de Renderização</h2>
          <span class="queue-count">{delivery.jobs.length}</span>
        </div>
        <div class="delivery-scroll">
          <Show
            when={delivery.jobs.length}
            fallback={
              <div class="queue-empty">
                <Download size={38} />
                <strong>Pronto para entregar</strong>
                <p>
                  Escolha um preset, defina o destino e adicione sua sequência à
                  fila.
                </p>
              </div>
            }
          >
            <For each={delivery.jobs}>
              {(job) => (
                <article
                  class="render-job"
                  classList={{
                    completed: job.status === "completed",
                    failed: job.status === "error",
                  }}
                >
                  <header>
                    <strong>
                      #{job.id} · {job.name}
                    </strong>
                    <span class={`job-status ${job.status}`}>
                      {statusName[job.status]}
                    </span>
                  </header>
                  <p>
                    {delivery.presets.find((p) => p.id === job.settings.preset)
                      ?.name ?? job.settings.preset}{" "}
                    · {job.settings.width}×{job.settings.height}
                  </p>
                  <small>
                    {job.settings.codec} · {job.settings.frameRate.numerator}/
                    {job.settings.frameRate.denominator} fps
                  </small>
                  <div class="job-destination" title={job.settings.output}>
                    {job.settings.output}
                  </div>
                  <progress
                    max="100"
                    value={job.percentage}
                    aria-label={`Progresso do trabalho ${job.id}`}
                  />
                  <div class="job-metrics">
                    <strong>{job.percentage.toFixed(2)}%</strong>
                    <span>{job.renderFps.toFixed(1)} fps</span>
                    <span>
                      ETA{" "}
                      {job.etaSeconds === null
                        ? "—"
                        : `${Math.ceil(job.etaSeconds)} s`}
                    </span>
                  </div>
                  <small>
                    {job.currentFrame} / {job.totalFrames} frames
                    {job.status === "rendering" &&
                    job.currentFrame === job.totalFrames
                      ? " · Finalizando arquivo…"
                      : ""}
                  </small>
                  <Show when={job.error}>
                    <p class="job-error">{job.error}</p>
                  </Show>
                  <Show
                    when={job.status === "ready" || job.status === "rendering"}
                  >
                    <button
                      class="job-cancel"
                      onClick={() => void cancelJob(job.id).catch(report)}
                    >
                      <X size={14} />
                      Cancelar
                    </button>
                  </Show>
                  <Show when={job.status === "completed"}>
                    <span class="job-done">
                      <CheckCircle2 size={14} />
                      Arquivo gravado
                    </span>
                  </Show>
                </article>
              )}
            </For>
          </Show>
        </div>
        <div class="queue-footer">
          <p>
            A fila usa snapshots independentes. Você pode continuar editando.
          </p>
          <button
            class="delivery-add"
            disabled={!delivery.jobs.some((j) => j.status === "ready")}
            onClick={() => {
              pausePlayback();
              void startQueue().catch(report);
            }}
          >
            <Play size={16} />
            Iniciar Render
          </button>
        </div>
      </aside>
    </main>
  );
}
