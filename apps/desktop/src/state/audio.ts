import { createStore, reconcile } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AudioStateDto,
  AudioPeaksDto,
  ChannelMixDto,
} from "../../../../contracts/ipc";
import { editor, acceptSnapshot, report } from "./editor";
import * as ipc from "../lib/ipc";
export const defaultChannel = (): ChannelMixDto => ({
  gainDb: 0,
  muted: false,
  solo: false,
  eqDb: [0, 0, 0],
});
export const [audio, setAudio] = createStore({
  state: {
    available: false,
    preparing: false,
    error: null,
    device: "",
    underruns: "0",
    latencyMs: 0,
    peakVersion: "0",
    meters: [],
  } as AudioStateDto,
  peaks: {} as Record<string, AudioPeaksDto>,
  selected: "master",
  drafts: {} as Record<string, ChannelMixDto>,
});
export const channel = (id: string): ChannelMixDto =>
  audio.drafts[id] ??
  (id === "master"
    ? editor.snapshot.timeline.mixer.master
    : editor.snapshot.timeline.mixer.tracks[id]) ??
  defaultChannel();
const pending = new Map<string, ChannelMixDto>();
let sending = false;
let timer: ReturnType<typeof setTimeout> | undefined;
export function changeChannel(id: string, patch: Partial<ChannelMixDto>) {
  if (!editor.connected) return;
  const next = { ...channel(id), ...patch };
  setAudio("drafts", id, next);
  pending.set(id, next);
  if (!timer)
    timer = setTimeout(() => {
      timer = undefined;
      void flush();
    }, 35);
}
async function flush() {
  if (sending) return;
  sending = true;
  try {
    while (pending.size) {
      const [id, next] = pending.entries().next().value!;
      pending.delete(id);
      try {
        const snapshot = await ipc.applyEdit({
          protocolVersion: 1,
          requestId: crypto.randomUUID(),
          expectedRevision: editor.snapshot.revision,
          edit: {
            kind: "mixer",
            track: id === "master" ? null : id,
            channel: next,
          },
        });
        acceptSnapshot(snapshot);
      } catch (e) {
        report(e);
        acceptSnapshot(await ipc.getState());
      } finally {
        if (!pending.has(id)) setAudio("drafts", id, undefined!);
      }
    }
  } finally {
    sending = false;
  }
}
export async function connectAudio() {
  let stopped = false,
    version = "",
    loading = false;
  const refresh = async () => {
    if (loading || stopped) return;
    loading = true;
    const wanted = audio.state.peakVersion;
    try {
      const peaks =
        await invoke<Record<string, AudioPeaksDto>>("get_audio_peaks");
      if (!stopped) {
        setAudio("peaks", reconcile(peaks));
        version = wanted;
      }
    } catch (e) {
      report(e);
    } finally {
      loading = false;
    }
  };
  const receive = (state: AudioStateDto) => {
    if (stopped) return;
    setAudio("state", state);
    if (state.peakVersion !== version) void refresh();
  };
  const stop = await listen<AudioStateDto>("audio://state", (e) =>
    receive(e.payload),
  );
  receive(await invoke<AudioStateDto>("get_audio_state"));
  return () => {
    stopped = true;
    stop();
    clearTimeout(timer);
  };
}
export const eqResponse = (eq: [number, number, number]) =>
  invoke<[number, number][]>("get_eq_response", { eq });
