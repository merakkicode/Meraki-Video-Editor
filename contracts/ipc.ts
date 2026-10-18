/** Meraki M1 wire contract. Implemented by apps/desktop/src-tauri/src/dto.rs.
 * Decimal strings preserve Rust i64/u64/u128 values beyond JS Number.MAX_SAFE_INTEGER.
 * The host must parse, range-check and validate every DTO before constructing domain objects.
 */
export type Id = string;
export type Revision = string;
export type TimeDto = { numerator: string; denominator: number };
export type AnimatedProperty = "scale"|"x"|"y"|"opacity"|"volume"|"rotation";
export interface KeyframeDto { at:TimeDto; value:number; interpolation:"linear"|"smooth" }
export interface TitleDto { text:string; font:string; size:number; color:[number,number,number,number]; align:string; shadow:boolean }
export interface MotionDto { offset:TimeDto; keys:Partial<Record<AnimatedProperty,KeyframeDto[]>>; fadeIn:TimeDto; fadeOut:TimeDto; dissolveIn:TimeDto; title:TitleDto|null }
export interface TransformDto { rotation?: number; scale: number; x: number; y: number; opacity: number }
export interface EffectSettingsDto { brightness: number; contrast: number; saturation: number; radius: number; colorBypass: boolean; blurBypass: boolean; colorConnected: boolean; blurConnected: boolean }
export interface ChannelMixDto { gainDb: number | null; muted: boolean; solo: boolean; eqDb: [number, number, number] }
export interface MixerSettingsDto { tracks: Record<Id, ChannelMixDto>; master: ChannelMixDto }
export interface AudioMeterDto { id: string; peak: [number, number]; rms: [number, number] }
export interface AudioStateDto { available: boolean; preparing: boolean; error: string | null; device: string; underruns: string; latencyMs: number; peakVersion: string; meters: AudioMeterDto[] }
export interface AudioPeaksDto { blockFrames: number; sampleRate: number; frames: string; values: [number, number][] }
export interface MarkerDto { id:string; at:TimeDto; name:string; color:"cyan"|"violet"|"yellow"|"red" }
export type EditDto =
  | {kind:"trim";clip:Id;head:boolean;at:TimeDto;ripple:boolean}
  | {kind:"marker";marker:MarkerDto;remove:boolean}
  | { kind:"motion";clip:Id;motion:MotionDto }
  | { kind:"title";id:Id;at:TimeDto }
  | { kind: "visual"; clip: Id; transform: TransformDto; effects: EffectSettingsDto; motion?:MotionDto }
  | { kind: "mixer"; track: string | null; channel: ChannelMixDto }
  | { kind: "move"; clip: Id; to: TimeDto }
  | { kind: "split"; clip: Id; at: TimeDto; rightId: Id };
export interface CommandEnvelope {
  protocolVersion: 1;
  requestId: string;
  expectedRevision: Revision;
  edit: EditDto;
}
export type EditReply = SnapshotDto;
export interface IpcError {
  code: "conflict" | "invalid" | "locked" | "notFound" | "unavailable";
  message: string;
  revision: Revision;
}
export interface UndoEnvelope {
  protocolVersion: 1;
  requestId: string;
  expectedRevision: Revision;
}
export interface ClipDto {
  motion?:MotionDto;
  transform: TransformDto;
  id: Id;
  name: string;
  source: {
    asset: Id;
    streamIndex: number;
    kind: "video" | "audio";
    availableStart: TimeDto;
    availableDuration: TimeDto;
  };
  timelineStart: TimeDto;
  duration: TimeDto;
  sourceIn: TimeDto;
  speed: TimeDto;
  enabled: boolean;
  linkGroup: Id | null;
  effects: Id | null;
}
export interface TrackDto {
  id: Id;
  name: string;
  kind: "video" | "audio";
  locked: boolean;
  enabled: boolean;
  solo: boolean;
  clips: ClipDto[];
}
export type SignalDto = "image" | "mask" | "scalar";
export type ValueDto =
  | { kind: "scalar"; value: number }
  | { kind: "vec2" | "color"; value: number[] }
  | { kind: "bool"; value: boolean };
export interface PortDto {
  node: Id;
  socket: number;
}
export interface GraphDto {
  id: Id;
  nodes: {
    bypass: boolean;
    id: Id;
    effect: string;
    inputs: SignalDto[];
    outputs: SignalDto[];
    parameters: Record<
      string,
      {
        value: ValueDto;
        keys: {
          at: TimeDto;
          value: ValueDto;
          interpolation: "hold" | "linear";
        }[];
      }
    >;
  }[];
  edges: { from: PortDto; to: PortDto }[];
  output: PortDto;
}
export interface TimelineDto {
  markers?: MarkerDto[];
  mixer: MixerSettingsDto;
  id: Id;
  name: string;
  frameRate: { numerator: number; denominator: number };
  audioSampleRate: number;
  tracks: TrackDto[];
  graphs: GraphDto[];
}
export interface SnapshotDto {
  protocolVersion: 1;
  revision: Revision;
  timeline: TimelineDto;
  assets: AssetDto[];
}
export interface AssetDto {
  audio_only: boolean;
  id: Id;
  path: string;
  name: string;
  width: number;
  height: number;
  preview_width: number;
  preview_height: number;
  frame_rate: { numerator: number; denominator: number };
  duration: TimeDto;
  stream_index: number;
  time_base: TimeDto;
  start_time: TimeDto;
  codec: string;
}
export interface TransportDto {
  compositor?: string;
  frame: string;
  playing: boolean;
  generation: string;
  acceptedSequence: string;
  revision: string;
  presentedSequence: string | null;
  presentedTime: TimeDto | null;
  sourcePts: TimeDto | null;
  buffering: boolean;
  error: string | null;
  cacheBytes: number;
  decoded: string;
  discarded: string;
  decodeMs: number;
}
export interface SeekRequest {
  requestId: string;
  sequence: string;
  timelineRevision: Revision;
  target: TimeDto;
}
export interface PreviewState {
  timelineRevision: Revision;
  generation: string;
  acceptedSeekSequence: string;
  presentedTime: TimeDto;
  mode: "native" | "binaryProxy";
  droppedFrames: number;
}
export interface PreviewViewport {
  /** Coordinates relative to the client area, in CSS pixels. Host converts once using DPI. */
  x: number;
  y: number;
  width: number;
  height: number;
  devicePixelRatio: number;
  visible: boolean;
}
export interface ExportSettingsDto {
  preset: string; output: string; container: "mp4" | "mov" | "mkv";
  width: number; height: number; frameRate: { numerator: number; denominator: number };
  codec: "libx264" | "libx265" | "prores_ks";
  rateMode: "crf" | "bitrate" | "quality"; crf: number; bitrateMbps: number;
  audioCodec: "aac" | "pcm_s24le" | "none"; audioBitrateKbps: number; sampleRate: number; channels: number;
}
export interface ExportPresetDto { id: string; name: string; description: string; settings: ExportSettingsDto }
export interface TimelineRangeDto { startFrame: string; endFrame: string }
export interface RenderJobDto {
  id: string; version: string; name: string; settings: ExportSettingsDto; timelineRange: TimelineRangeDto;
  status: "ready" | "rendering" | "completed" | "cancelled" | "error";
  currentFrame: string; totalFrames: string; percentage: number; renderFps: number; etaSeconds: number | null; error: string | null;
}
export interface RenderProgressDto {
  job_id: string; current_frame: string; total_frames: string; percentage: number;
  render_fps: number; eta_seconds: number | null; job: RenderJobDto;
}
