import { expect, test } from "vitest";
import type { ClipDto, GraphDto } from "../../../../contracts/ipc";
import { defaultEffects, readVisual } from "./visual";
test("restores clip transforms and distinguishes disconnected processors from bypass", () => {
  const clip = {
    effects: "90071992547409930",
    transform: { scale: 0.5, x: 420, y: 210, opacity: 0.65 },
  } as ClipDto;
  const node = (
    id: string,
    effect: string,
    parameters: GraphDto["nodes"][number]["parameters"] = {},
    bypass = false,
  ) => ({ id, effect, inputs: [], outputs: [], parameters, bypass });
  const graph: GraphDto = {
    id: clip.effects!,
    nodes: [
      node("1", "source@1"),
      node("2", "color@1", {
        contrast: { value: { kind: "scalar", value: 1.4 }, keys: [] },
      }),
      node(
        "3",
        "blur@1",
        { radius: { value: { kind: "scalar", value: 15 }, keys: [] } },
        true,
      ),
      node("4", "output@1"),
    ],
    edges: [
      { from: { node: "1", socket: 0 }, to: { node: "2", socket: 0 } },
      { from: { node: "1", socket: 0 }, to: { node: "3", socket: 0 } },
      { from: { node: "3", socket: 0 }, to: { node: "4", socket: 0 } },
    ],
    output: { node: "4", socket: 0 },
  };
  const visual = readVisual(clip, [graph]);
  expect(visual.transform).toEqual(clip.transform);
  expect(visual.effects).toEqual({
    ...defaultEffects(),
    contrast: 1.4,
    radius: 15,
    colorConnected: false,
    blurBypass: true,
  });
  expect(readVisual(undefined, []).effects).toEqual(defaultEffects());
});
