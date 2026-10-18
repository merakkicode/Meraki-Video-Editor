import { createStore } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { acceptSnapshot, editor, report, seek, setEditor } from "./editor";
import { flushVisual } from "./visual";
import type { SnapshotDto } from "../../../../contracts/ipc";
interface MediaStatus {
  offline: string[];
  proxies: string[];
  enabled: boolean;
  busy: boolean;
  message: string;
}
export const [workflow, setWorkflow] = createStore({
  offline: [] as string[],
  proxies: [] as string[],
  enabled: false,
  busy: false,
  message: "",
  recovery: [] as string[],
  recovering: false,
  autosave: "",
  context: null as { id: string; x: number; y: number } | null,
});
let polling = false;
export async function refreshMedia() {
  if (polling || !editor.connected) return;
  polling = true;
  try {
    const status = await invoke<MediaStatus>("media_status");
    const changed = status.offline.join() !== workflow.offline.join();
    setWorkflow(status);
    if (changed) seek(editor.frame);
  } catch (e) {
    report(e);
  } finally {
    polling = false;
  }
}
export async function connectWorkflow() {
  await refreshMedia();
  try {
    setWorkflow("recovery", await invoke<string[]>("recovery_list"));
  } catch (e) {
    report(e);
  }
  const autosave = await listen<string>("project://autosave", (e) => {
    if (e.payload) setWorkflow("autosave", e.payload);
  });
  const proxy = await listen<string>(
    "media://proxy",
    () => void refreshMedia(),
  );
  const timer = setInterval(() => void refreshMedia(), 5000);
  return () => {
    clearInterval(timer);
    autosave();
    proxy();
  };
}
export function mediaContext(e: MouseEvent, id: string) {
  e.preventDefault();
  e.stopPropagation();
  setWorkflow("context", {
    id,
    x: Math.min(e.clientX, window.innerWidth - 230),
    y: Math.min(e.clientY, window.innerHeight - 140),
  });
}
export async function toggleProxies() {
  try {
    await invoke("set_proxies", { enabled: !workflow.enabled });
    await refreshMedia();
  } catch (e) {
    report(e);
  }
}
export async function generateProxy(id: string) {
  setWorkflow("context", null);
  try {
    await invoke("generate_proxy", { assetId: id });
    await refreshMedia();
  } catch (e) {
    report(e);
  }
}
export async function relink(id: string) {
  setWorkflow("context", null);
  if (editor.busy) return;
  setEditor({ busy: true, error: "" });
  try {
    await flushVisual();
    const s = await invoke<SnapshotDto | null>("relink_media", {
      assetId: id,
      expectedRevision: editor.snapshot.revision,
    });
    if (s) {
      acceptSnapshot(s);
      seek(editor.frame);
      setEditor(
        "notice",
        "Mídia reconectada. Tempos preservados; histórico de Undo reiniciado.",
      );
    }
    await refreshMedia();
  } catch (e) {
    report(e);
  } finally {
    setEditor("busy", false);
  }
}
export async function recover(name: string, restore: boolean) {
  if (workflow.recovering) return;
  setWorkflow("recovering", true);
  setEditor("error", "");
  try {
    const s = await invoke<SnapshotDto | null>("recovery_action", {
      name,
      recover: restore,
      expectedRevision: editor.snapshot.revision,
    });
    if (s) {
      acceptSnapshot(s);
      seek(0n);
      setEditor(
        "notice",
        "Alterações recuperadas. Use Ctrl+S para salvar seu projeto.",
      );
    }
    setWorkflow("recovery", restore ? [] : await invoke<string[]>("recovery_list"));
    await refreshMedia();
  } catch (e) {
    report(e);
  } finally {
    setWorkflow("recovering", false);
  }
}
