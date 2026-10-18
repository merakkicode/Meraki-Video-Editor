import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CommandEnvelope,
  SnapshotDto,
  UndoEnvelope,
  PreviewViewport,
  TransportDto,
} from "../../../../contracts/ipc";
export const native = isTauri();
export const getState = () => invoke<SnapshotDto>("get_state");
export const applyEdit = (envelope: CommandEnvelope) =>
  invoke<SnapshotDto>("apply_edit", { envelope });
export const undoEdit = (envelope: UndoEnvelope) =>
  invoke<SnapshotDto>("undo", { envelope });
export const subscribe = (receive: (s: SnapshotDto) => void) =>
  listen<SnapshotDto>("engine://snapshot", (e) => receive(e.payload));
export const viewport = (geometry: PreviewViewport) =>
  native
    ? invoke<void>("set_preview_viewport", { viewport: geometry })
    : Promise.resolve();
export const importMedia = (expectedRevision: string) =>
  invoke<SnapshotDto | null>("import_media", { expectedRevision });
export const insertMedia = (assetId: string, expectedRevision: string, trackId?: string) =>
  invoke<SnapshotDto>("insert_media", { assetId, expectedRevision, trackId });
export const openProject = (expectedRevision: string) =>
  invoke<SnapshotDto | null>("open_project", { expectedRevision });
export const saveProject = () => invoke<string | null>("save_project");
export const getTransport = () => invoke<TransportDto>("get_transport");
export const transport = (command: {
  sequence: string;
  revision: string;
  action: string;
  frame?: string;
}) => invoke<TransportDto>("transport_command", { command });
export const subscribeTransport = (receive: (state: TransportDto) => void) =>
  listen<TransportDto>("transport://state", (e) => receive(e.payload));
