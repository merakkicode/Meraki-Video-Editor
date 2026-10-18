import { flushVisual } from "./visual";
import { connectAudio, channel, changeChannel } from "./audio";
import { createStore, reconcile } from "solid-js/store";
import type {
  ClipDto,
  EditDto,
  IpcError,
  SnapshotDto,
  TrackDto,
} from "../../../../contracts/ipc";
import * as ipc from "../lib/ipc";
import { decimal, frameAt, frameTime, newId } from "../lib/time";
export type Space = "montagem" | "trama" | "som" | "entrega";
function demo(): SnapshotDto {
  return {
    protocolVersion: 1,
    revision: "0",
    assets: [],
    timeline: {
      id: "1",
      name: "Novo projeto Meraki",
      frameRate: { numerator: 30, denominator: 1 },
      audioSampleRate: 48000,
      graphs: [],
      mixer: {
        tracks: {},
        master: { gainDb: 0, muted: false, solo: false, eqDb: [0, 0, 0] },
      },
      tracks: [1, 2, 3, 4].map((i) => ({
        id: String(i),
        name: `${i <= 2 ? "Vídeo" : "Áudio"} ${i <= 2 ? i : i - 2}`,
        kind: i <= 2 ? "video" : "audio",
        locked: false,
        enabled: true,
        solo: false,
        clips: [],
      })),
    },
  };
}
const initialTransport: import("../../../../contracts/ipc").TransportDto = {
  frame: "0",
  playing: false,
  generation: "0",
  acceptedSequence: "0",
  revision: "0",
  presentedSequence: null,
  presentedTime: null,
  sourcePts: null,
  buffering: false,
  error: null,
  cacheBytes: 0,
  decoded: "0",
  discarded: "0",
  decodeMs: 0,
};
export const [editor, setEditor] = createStore({
  snapshot: demo(),
  connected: false,
  loading: ipc.native,
  busy: false,
  space: "montagem" as Space,
  selected: "1",
  tool: "select" as "select" | "cut" | "trim",
  snap: true,
  frame: 0n,
  transport: initialTransport,
  transportPending: false,
  playing: false,
  inFrame: 0n,
  outFrame: 0n,
  notice: "",
  error: "",
  zoom: 1,
  trackControls: {} as Record<
    string,
    { muted: boolean; solo: boolean; locked: boolean }
  >,
});
export const selectedClip = () =>
  editor.snapshot.timeline.tracks
    .flatMap((t) => t.clips)
    .find((c) => c.id === editor.selected);
export const rate = () => editor.snapshot.timeline.frameRate;
export const canEdit = () => editor.connected && !editor.busy;
export function acceptSnapshot(snapshot: SnapshotDto) {
  if (snapshot.protocolVersion !== 1)
    throw new Error("Versão IPC incompatível.");
  if (
    decimal(snapshot.revision, (1n << 64n) - 1n) <
    decimal(editor.snapshot.revision, (1n << 64n) - 1n)
  )
    return;
  setEditor("snapshot", reconcile(snapshot));
  setEditor(
    "outFrame",
    snapshot.timeline.tracks
      .flatMap((t) => t.clips)
      .reduce((max, c) => {
        const end =
          frameAt(c.timelineStart, rate()) + frameAt(c.duration, rate());
        return end > max ? end : max;
      }, 0n),
  );
  if (!selectedClip())
    setEditor(
      "selected",
      snapshot.timeline.tracks.flatMap((t) => t.clips)[0]?.id ?? "",
    );
}
export function report(error: unknown) {
  const e = error as Partial<IpcError>;
  setEditor(
    "error",
    e.message ||
      (typeof error === "string" ? error : "Não foi possível concluir a ação."),
  );
}
export async function initialize(): Promise<() => void> {
  if (!ipc.native) {
    setEditor("notice", "Prévia web · edição disponível no aplicativo desktop");
    return () => {};
  }
  let unlisten: (() => void) | undefined;
  try {
    unlisten = await ipc.subscribe((s) => {
      try {
        acceptSnapshot(s);
      } catch (e) {
        report(e);
      }
    });
    const stopTransport = await ipc.subscribeTransport(acceptTransport);
    const stopAudio = await connectAudio();
    const stopSnapshot = unlisten;
    unlisten = () => {
      stopSnapshot?.();
      stopTransport();
      stopAudio();
    };
    acceptSnapshot(await ipc.getState());
    acceptTransport(await ipc.getTransport());
    setEditor("connected", true);
  } catch (e) {
    unlisten?.();
    report(e);
  } finally {
    setEditor("loading", false);
  }
  return () => unlisten?.();
}
async function mutation(work: () => Promise<SnapshotDto>) {
  if (!canEdit()) return;
  setEditor({ busy: true, error: "", playing: false });
  try {
    await flushVisual();
    acceptSnapshot(await work());
  } catch (e) {
    report(e);
    if ((e as IpcError).code === "conflict") {
      try {
        acceptSnapshot(await ipc.getState());
      } catch (fetchError) {
        report(fetchError);
      }
    }
  } finally {
    setEditor("busy", false);
  }
}
export async function edit(action: EditDto) {
  const track = editor.snapshot.timeline.tracks.find((t) =>
    t.clips.some((c) => "clip" in action && c.id === action.clip),
  );
  if (track && (track.locked || editor.trackControls[track.id]?.locked)) {
    setEditor("error", "Trilha bloqueada nesta sessão.");
    return;
  }
  await mutation(() =>
    ipc.applyEdit({
      protocolVersion: 1,
      requestId: crypto.randomUUID(),
      expectedRevision: editor.snapshot.revision,
      edit: action,
    }),
  );
}
export async function split(id = editor.selected, at = editor.frame) {
  await edit({
    kind: "split",
    clip: id,
    at: frameTime(at, rate()),
    rightId: newId(),
  });
}
export async function move(id: string, frame: bigint) {
  await edit({
    kind: "move",
    clip: id,
    to: frameTime(frame < 0n ? 0n : frame, rate()),
  });
}
export async function undo() {
  await mutation(() =>
    ipc.undoEdit({
      protocolVersion: 1,
      requestId: crypto.randomUUID(),
      expectedRevision: editor.snapshot.revision,
    }),
  );
}
export function select(id: string) {
  setEditor("selected", id);
}
export function seek(frame: bigint) {
  const target = frame < 0n ? 0n : frame;
  setEditor({ frame: target, playing: false });
  queueTransport("seek", target);
}
export function toggleTrack(id: string, key: "muted" | "solo" | "locked") {
  if (key !== "locked") {
    changeChannel(id, { [key]: !channel(id)[key] });
    return;
  }
  const prev = editor.trackControls[id] ?? {
    muted: false,
    solo: false,
    locked: false,
  };
  setEditor("trackControls", id, { ...prev, [key]: !prev[key] });
}
export function selectedFrame() {
  const clip = selectedClip();
  return clip ? frameAt(clip.timelineStart, rate()) : 0n;
}

let commandSequence = 0n;
let pendingCommand: {
  sequence: string;
  revision: string;
  action: string;
  frame?: string;
} | null = null;
let sendingTransport = false;
function acceptTransport(
  state: import("../../../../contracts/ipc").TransportDto,
) {
  if (
    state.revision !== editor.snapshot.revision ||
    BigInt(state.acceptedSequence) < commandSequence ||
    BigInt(state.generation) < BigInt(editor.transport.generation)
  )
    return;
  commandSequence = BigInt(state.acceptedSequence);
  setEditor({
    transport: state,
    frame: BigInt(state.frame),
    playing: state.playing,
    transportPending: false,
  });
}
function queueTransport(action: string, frame?: bigint) {
  if (!ipc.native || !editor.connected) return;
  commandSequence += 1n;
  setEditor("transportPending", true);
  pendingCommand = {
    sequence: commandSequence.toString(),
    revision: editor.snapshot.revision,
    action,
    ...(frame === undefined ? {} : { frame: frame.toString() }),
  };
  void flushTransport();
}
async function flushTransport() {
  if (sendingTransport) return;
  sendingTransport = true;
  try {
    while (pendingCommand) {
      const cmd = pendingCommand;
      pendingCommand = null;
      try {
        acceptTransport(await ipc.transport(cmd));
      } catch (e) {
        report(e);
        setEditor("transportPending", false);
      }
    }
  } finally {
    sendingTransport = false;
  }
}
export function togglePlayback() {
  queueTransport(editor.playing ? "pause" : "play");
}
export function pausePlayback() {
  queueTransport("pause");
}
export async function importMedia() {
  await documentAction(async () => {
    const s = await ipc.importMedia(editor.snapshot.revision);
    if (s) {
      acceptSnapshot(s);
      setEditor(
        "selected",
        s.timeline.tracks.flatMap((t) => t.clips).at(-1)?.id ?? "",
      );
      seek(0n);
      setEditor(
        "notice",
        "Mídia importada. O áudio e as formas de onda são preparados em segundo plano.",
      );
    }
  });
}
export async function insertMedia(id: string, trackId?: string) {
  await documentAction(async () => {
    acceptSnapshot(await ipc.insertMedia(id, editor.snapshot.revision, trackId));
  });
}
export async function openProject() {
  await documentAction(async () => {
    const s = await ipc.openProject(editor.snapshot.revision);
    if (s) {
      acceptSnapshot(s);
      seek(0n);
      setEditor(
        "notice",
        "Projeto carregado. As referências de mídia são locais.",
      );
    }
  });
}
export async function saveProject() {
  await documentAction(async () => {
    const path = await ipc.saveProject();
    if (path) setEditor("notice", `Projeto salvo: ${path}`);
  });
}
async function documentAction(work: () => Promise<void>) {
  if (!canEdit()) return;
  setEditor({ busy: true, error: "", notice: "" });
  pausePlayback();
  try {
    await flushVisual();
    await work();
  } catch (e) {
    report(e);
    if ((e as IpcError).code === "conflict")
      acceptSnapshot(await ipc.getState());
  } finally {
    setEditor("busy", false);
  }
}


export async function createTitle(){const id=newId();await edit({kind:"title",id,at:frameTime(editor.frame,rate())});if(editor.snapshot.timeline.tracks.some(t=>t.clips.some(c=>c.id===id)))select(id);}
