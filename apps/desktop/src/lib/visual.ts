import { defaultMotion } from "./animation";
import type {
  ClipDto,
  EffectSettingsDto,
  GraphDto,
  TransformDto,
} from "../../../../contracts/ipc";
export const defaultTransform = (): TransformDto => ({
  rotation: 0,
  scale: 1,
  x: 0,
  y: 0,
  opacity: 1,
});
export const defaultEffects = (): EffectSettingsDto => ({
  brightness: 0,
  contrast: 1,
  saturation: 1,
  radius: 0,
  colorBypass: false,
  blurBypass: false,
  colorConnected: true,
  blurConnected: true,
});
export function readVisual(clip: ClipDto | undefined, graphs: GraphDto[]) {
  const effects = defaultEffects();
  const graph = graphs.find((g) => g.id === clip?.effects);
  if (graph) {
    const reachable = new Set<string>();
    let current: string | undefined = graph.output.node;
    while (current && !reachable.has(current)) {
      reachable.add(current);
      current = graph.edges.find((e) => e.to.node === current)?.from.node;
    }
    for (const node of graph.nodes) {
      if (node.effect === "color@1") {
        effects.colorConnected = reachable.has(node.id);
        effects.colorBypass = node.bypass;
        for (const key of ["brightness", "contrast", "saturation"] as const) {
          const p = node.parameters[key]?.value;
          if (p?.kind === "scalar") effects[key] = p.value;
        }
      } else if (node.effect === "blur@1") {
        effects.blurConnected = reachable.has(node.id);
        effects.blurBypass = node.bypass;
        const p = node.parameters.radius?.value;
        if (p?.kind === "scalar") effects.radius = p.value;
      }
    }
  }
  return { transform: { ...(clip?.transform ?? defaultTransform()) }, effects, motion:clip?.motion ?? defaultMotion() };
}
