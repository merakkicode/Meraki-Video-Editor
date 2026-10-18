import type {AnimatedProperty, MotionDto, TitleDto} from "../../../../contracts/ipc";
import {addTime,compareTime,evaluateKeys} from "../lib/animation";
import {frameTime} from "../lib/time";
import {rate} from "./editor";
import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";
import type {
  EffectSettingsDto,
  TransformDto,
} from "../../../../contracts/ipc";
import { editor, selectedClip, acceptSnapshot, report } from "./editor";
import * as ipc from "../lib/ipc";
import { readVisual } from "../lib/visual";
type Visual = ReturnType<typeof readVisual>;
const [drafts, setDrafts] = createStore<Record<string, Visual>>({});
export const [visualNode, setVisualNode] = createSignal(2);
export const rawVisual = () =>
  drafts[editor.selected] ??
  readVisual(selectedClip(), editor.snapshot.timeline.graphs);
export const canVisual = () =>
  editor.connected &&
  !editor.busy &&
  selectedClip()?.source.kind === "video" &&
  !editor.snapshot.timeline.tracks.some(
    (t) =>
      (t.locked || editor.trackControls[t.id]?.locked) &&
      t.clips.some((c) => c.id === editor.selected),
  );
const pending = new Map<string, Visual>();
let timer: ReturnType<typeof setTimeout> | undefined;
let running: Promise<void> | undefined;
export function changeVisual(
  transform: Partial<TransformDto> = {},
  effects: Partial<EffectSettingsDto> = {},
) {
  if (!canVisual()) return;
  const current = rawVisual();
  const next = {
    transform: { ...current.transform, ...transform },
    effects: { ...current.effects, ...effects },
    motion: {...current.motion,keys:{...current.motion.keys}},
  };
  for(const [p,value] of Object.entries(transform)) {
    const property=p as AnimatedProperty; if(next.motion.keys[property]?.length && canKey()) { next.motion.keys[property]=upsert(next.motion.keys[property]!,keyTime(),value); }
  }
  queue(next);
}
function queue(next:Visual) {
  setDrafts(editor.selected, next);
  pending.set(editor.selected, next);
  if (!timer)
    timer = setTimeout(() => {
      timer = undefined;
      void flushVisual();
    }, 40);
}
export function flushVisual(): Promise<void> {
  clearTimeout(timer);
  timer = undefined;
  if (running) return running;
  running = (async () => {
    while (pending.size) {
      const [id, next] = pending.entries().next().value!;
      pending.delete(id);
      try {
        acceptSnapshot(
          await ipc.applyEdit({
            protocolVersion: 1,
            requestId: crypto.randomUUID(),
            expectedRevision: editor.snapshot.revision,
            edit: editor.snapshot.timeline.tracks.some(t=>t.kind==="audio"&&t.clips.some(c=>c.id===id)) ? {kind:"motion",clip:id,motion:next.motion} : { kind: "visual", clip: id, ...next },
          }),
        );
      } catch (e) {
        report(e);
        try {
          acceptSnapshot(await ipc.getState());
        } catch (err) {
          report(err);
        }
      } finally {
        if (!pending.has(id)) setDrafts(id, undefined!);
      }
    }
  })().finally(() => {
    running = undefined;
  });
  return running;
}

export const [keyInterpolation,setKeyInterpolation]=createSignal<"linear"|"smooth">("smooth");
export function keyTime(){const c=selectedClip();if(!c)return frameTime(0n,rate());return addTime(addTime(frameTime(editor.frame,rate()),c.timelineStart,-1n),rawVisual().motion.offset);}
export function canKey(){const c=selectedClip();if(!c)return false;const at=addTime(frameTime(editor.frame,rate()),c.timelineStart,-1n);return compareTime(at,frameTime(0n,rate()))>=0&&compareTime(at,c.duration)<0;}
function upsert(keys:import("../../../../contracts/ipc").KeyframeDto[],at:import("../../../../contracts/ipc").TimeDto,value:number){return [...keys.filter(k=>compareTime(k.at,at)!==0),{at,value,interpolation:keyInterpolation()}].sort((a,b)=>compareTime(a.at,b.at));}
export function visual(){const v=rawVisual();const t={...v.transform};for(const p of ["scale","x","y","opacity","rotation"] as const)t[p]=evaluateKeys(v.motion.keys[p]??[],keyTime(),t[p]??0);return {...v,transform:t};}
export function hasKey(p:AnimatedProperty){return (rawVisual().motion.keys[p]??[]).some(k=>compareTime(k.at,keyTime())===0);}
export function changeMotion(patch:Partial<MotionDto>){if(!editor.connected||editor.busy||!selectedClip()||editor.snapshot.timeline.tracks.some(t=>(t.locked||editor.trackControls[t.id]?.locked)&&t.clips.some(c=>c.id===editor.selected)))return;queue({...rawVisual(),motion:{...rawVisual().motion,...patch}});}
export function toggleKey(p:AnimatedProperty){if(!canKey())return;const m=rawVisual().motion;const value=p==="volume"?volumeValue():visual().transform[p]??0;changeMotion({keys:{...m.keys,[p]:hasKey(p)?(m.keys[p]??[]).filter(k=>compareTime(k.at,keyTime())!==0):upsert(m.keys[p]??[],keyTime(),value)}});}
export function volumeValue(){return evaluateKeys(rawVisual().motion.keys.volume??[],keyTime(),1);}
export function changeVolume(value:number){if(canKey())changeMotion({keys:{...rawVisual().motion.keys,volume:upsert(rawVisual().motion.keys.volume??[],keyTime(),value)}});}
export function changeTitle(patch:Partial<TitleDto>){const t=rawVisual().motion.title;if(t)changeMotion({title:{...t,...patch}});}
