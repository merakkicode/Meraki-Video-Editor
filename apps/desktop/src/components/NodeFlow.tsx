import { createSignal, For } from "solid-js";
import {
  Workflow,
  ZoomIn,
  ZoomOut,
  Maximize,
  Film,
  SlidersHorizontal,
  Circle,
  Monitor,
} from "lucide-solid";
import { selectedClip } from "../state/editor";
import {
  visual,
  changeVisual,
  canVisual,
  visualNode as selected,
  setVisualNode as setSelected,
} from "../state/visual";
export default function NodeFlow() {
  const initial = [
    {
      id: 1,
      name: "MÃ­dia",
      detail: "Costa ao amanhecer",
      x: 60,
      y: 110,
      icon: Film,
    },
    {
      id: 2,
      name: "Ajuste de Cor",
      detail: "PrimÃ¡rias Â· Rec.709",
      x: 325,
      y: 70,
      icon: SlidersHorizontal,
    },
    {
      id: 3,
      name: "Desfoque",
      detail: "Gaussiano Â· 12 px",
      x: 590,
      y: 130,
      icon: Circle,
    },
    {
      id: 4,
      name: "SaÃ­da",
      detail: "SequÃªncia principal",
      x: 855,
      y: 90,
      icon: Monitor,
    },
  ];
  const [nodes, setNodes] = createSignal(initial),
    [zoom, setZoom] = createSignal(1);
  const connected = (id: number) =>
    id === 2
      ? visual().effects.colorConnected
      : id === 3
        ? visual().effects.blurConnected
        : true;
  const bypassed = (id: number) =>
    id === 2
      ? visual().effects.colorBypass
      : id === 3
        ? visual().effects.blurBypass
        : false;
  const route = () => nodes().filter((n) => connected(n.id));
  const links = () => {
    const active = route();
    const pairs = active.slice(0, -1).map((n, i) => [n, active[i + 1]]);
    for (const n of nodes().filter((n) => !connected(n.id)))
      pairs.push([nodes()[0], n]);
    return pairs;
  };
  const detail = (id: number) =>
    id === 1
      ? (selectedClip()?.name ?? "Selecione um clipe")
      : id === 2
        ? `Brilho ${visual().effects.brightness.toFixed(2)} Â· Contraste ${visual().effects.contrast.toFixed(2)}`
        : id === 3
          ? `Raio Â· ${visual().effects.radius} px`
          : "ComposiÃ§Ã£o por trilhas";
  function drag(e: PointerEvent, id: number) {
    if (e.button !== 0) return;
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    setSelected(id);
    const node = nodes().find((n) => n.id === id)!;
    const sx = e.clientX,
      sy = e.clientY;
    const update = (ev: PointerEvent) =>
      setNodes((ns) =>
        ns.map((n) =>
          n.id === id
            ? {
                ...n,
                x: Math.max(
                  10,
                  Math.min(1020, node.x + (ev.clientX - sx) / zoom()),
                ),
                y: Math.max(
                  10,
                  Math.min(300, node.y + (ev.clientY - sy) / zoom()),
                ),
              }
            : n,
        ),
      );
    const stop = () => {
      window.removeEventListener("pointermove", update);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
    };
    window.addEventListener("pointermove", update);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }
  return (
    <section class="panel flow-panel">
      <div class="panel-title">
        <h2>
          <Workflow size={16} />
          Fluxo <span class="quiet">/ Efeitos por nÃ³s</span>
        </h2>
        <div class="inline">
          <button
            disabled={!canVisual() || ![2, 3].includes(selected())}
            onClick={() =>
              changeVisual(
                {},
                selected() === 2
                  ? { colorConnected: !connected(2) }
                  : { blurConnected: !connected(3) },
              )
            }
          >
            {connected(selected()) ? "Desconectar" : "Reconectar"} nÃ³
          </button>
          <button
            aria-pressed={bypassed(selected())}
            disabled={!canVisual() || ![2, 3].includes(selected())}
            onClick={() =>
              changeVisual(
                {},
                selected() === 2
                  ? { colorBypass: !bypassed(2) }
                  : { blurBypass: !bypassed(3) },
              )
            }
          >
            Bypass
          </button>
        </div>
      </div>
      <div class="flow-scroll">
        <div
          class="flow-canvas"
          style={{ width: `${1200 * zoom()}px`, height: `${380 * zoom()}px` }}
        >
          <div class="flow-world" style={{ transform: `scale(${zoom()})` }}>
            <svg class="connections" viewBox="0 0 1200 380">
              <defs>
                <linearGradient id="connection-gradient">
                  <stop stop-color="#00d2ff" />
                  <stop offset="1" stop-color="#9d4edd" />
                </linearGradient>
              </defs>
              <For each={links()}>
                {([node, target]) => {
                  const next = () => target;
                  return (
                    <path
                      d={`M${node.x + 190},${node.y + 44} C${node.x + 245},${node.y + 44} ${next().x - 70},${next().y + 44} ${next().x},${next().y + 44}`}
                    />
                  );
                }}
              </For>
            </svg>
            <For each={nodes()}>
              {(node) => (
                <button
                  class="flow-node"
                  classList={{
                    selected: selected() === node.id,
                    bypassed: bypassed(node.id) || !connected(node.id),
                  }}
                  style={{ left: `${node.x}px`, top: `${node.y}px` }}
                  onPointerDown={(e) => drag(e, node.id)}
                  onClick={() => setSelected(node.id)}
                >
                  <div class="node-title">
                    <node.icon size={16} />
                    {node.name}
                    <span>{String(node.id).padStart(2, "0")}</span>
                  </div>
                  <small>
                    {!connected(node.id)
                      ? "Fora da saÃ­da Â· "
                      : bypassed(node.id)
                        ? "Bypass Â· "
                        : ""}
                    {detail(node.id)}
                  </small>
                  <span class="node-port input" />
                  <span class="node-port output" />
                </button>
              )}
            </For>
          </div>
        </div>
      </div>
      <div class="flow-bottom">
        <span>
          <span class="status-dot" />
          <span class="flow-label">
            Cor â†’ Desfoque â†’ Transformar â†’ Mesclar Â·{" "}
            {selectedClip()?.name ?? "Selecione um vÃ­deo"}
          </span>
        </span>
        <div class="inline">
          <button
            class="icon-button"
            title="Reduzir zoom"
            onClick={() => setZoom(Math.max(0.5, zoom() - 0.1))}
          >
            <ZoomOut size={15} />
          </button>
          <span>{Math.round(zoom() * 100)}%</span>
          <button
            class="icon-button"
            title="Aumentar zoom"
            onClick={() => setZoom(Math.min(1.5, zoom() + 0.1))}
          >
            <ZoomIn size={15} />
          </button>
          <button
            class="icon-button"
            title="Restaurar layout"
            onClick={() => {
              setNodes(initial);
              setZoom(1);
            }}
          >
            <Maximize size={14} />
          </button>
        </div>
      </div>
    </section>
  );
}
