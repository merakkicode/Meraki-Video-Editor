import { createStore } from "solid-js/store";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  ExportPresetDto,
  ExportSettingsDto,
  RenderJobDto,
  RenderProgressDto,
  TimelineRangeDto,
} from "../../../../contracts/ipc";
import { native } from "../lib/ipc";
export const [delivery, setDelivery] = createStore({
  jobs: [] as RenderJobDto[],
  presets: [] as ExportPresetDto[],
});
export function acceptJob(job: RenderJobDto) {
  const i = delivery.jobs.findIndex((j) => j.id === job.id);
  if (i < 0) setDelivery("jobs", (j) => [...j, job]);
  else if (BigInt(job.version) >= BigInt(delivery.jobs[i].version))
    setDelivery("jobs", i, job);
}
export async function connectDelivery() {
  if (!native) return () => {};
  const stop = await listen<RenderProgressDto>("render://progress", (e) =>
    acceptJob(e.payload.job),
  );
  try {
    const [jobs, presets] = await Promise.all([
      invoke<RenderJobDto[]>("get_render_jobs"),
      invoke<ExportPresetDto[]>("get_export_presets"),
    ]);
    jobs.forEach(acceptJob);
    setDelivery("presets", presets);
    return stop;
  } catch (e) {
    stop();
    throw e;
  }
}
export const addJob = async (
  settings: ExportSettingsDto,
  timelineRange: TimelineRangeDto,
) =>
  acceptJob(
    await invoke<RenderJobDto>("add_render_job", { settings, timelineRange }),
  );
export const startQueue = () => invoke("start_render_queue");
export const cancelJob = (jobId: string) =>
  invoke("cancel_render_job", { jobId });
export const browseFolder = () => invoke<string | null>("browse_export_folder");
