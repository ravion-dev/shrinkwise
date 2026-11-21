import { t, tn } from "./i18n";

export interface Settings {
  lossy: number;
  losslessMode: boolean;
  simpleMode: boolean;
  workers: number;
  keepLarger: boolean;
  keepMetadata: boolean;
  keepMtime: boolean;
  minSizeKb: number;
  limitMb: number;
  excludeGlobs: string;
  onlyKinds: string[];
  onlyStrict: boolean;
  dryRun: boolean;
  verify: boolean;
  deepImage: boolean;

  theme: "dark" | "light";
  fontScale: number;
  language: string;
  showFileList: boolean;
  notifyDone: boolean;
  autoCheckTools: boolean;

  imageFormat: string;
  convertPngToJpeg: boolean;
  convertBmpToJpeg: boolean;
  imageMaxMp: number;
  imageQuality: number;
  imageMaxSidePct: number;
  imageAlpha: string;
  extraWebpToo: boolean;

  gifMode: string;
  gifMaxFps: string;
  gifMaxWidth: number;
  gifLoop: boolean;

  videoCodec: string;
  encoderPrefer: string;
  videoResolution: string;
  videoMaxFps: string;
  videoAudioCodec: string;
  videoScalePct: number;
  video10bit: boolean;
  videoScaleSearch: string;
  videoTwoPass: string;
  videoTune: string;
  videoDenoise: boolean;
  videoHdrTonemap: boolean;
  videoFaststart: boolean;
  videoKeepSubs: boolean;
  videoDeepVerify: boolean;

  audioNormalize: boolean;
  audioMono: boolean;
  audioTrimSilence: boolean;

  pdfMode: string;
  pdfRasterDpi: number;
  officeMode: string;
  archiveMode: string;
  textMode: string;
  svgMode: string;
  fontMode: string;
  zipDeep: boolean;
  zipDeepDepth: number;

  targetMode: string;
  targetImageKb: number;
  targetVideoMb: number;
  targetAudioKb: number;
  targetGifKb: number;
  targetPdfKb: number;
  targetTotalMb: number;
  targetMinQuality: number;

  smartMode: boolean;
  smartMinSsim: number;
  smartMaxEncodes: number;
  smartScaleSearch: boolean;
  smartContentAnalysis: boolean;
  smartGifRace: boolean;
  smartAudioGuard: boolean;

  gpu: boolean;
  gpuDecoder: string;
  gpuDevice: number;
  gpuAv1: boolean;
  ffmpegPath: string;
  ffprobePath: string;
  sevenZipPath: string;
  ghostscriptPath: string;
  qpdfPath: string;
}

export interface CpuInfo {
  brand: string;
  vendor: string;
  physicalCores: number;
  logicalCores: number;
  frequencyMhz: number;
}

export interface GpuInfo {
  name: string;
  vendor: string;
  vendorLabel: string;
  memoryMb: number;
  driver: string;
  software: boolean;
}

export interface SystemInfo {
  os: string;
  arch: string;
  cpu: CpuInfo;
  memoryTotalMb: number;
  memoryAvailableMb: number;
  gpus: GpuInfo[];
  recommendedWorkers: number;
  recommendedEncoder: string;
}

export interface CpuBenchmark {
  scoreMbps: number;
  elapsedMs: number;
  recommendedWorkers: number;
  label: string;
}

export interface ToolInfo {
  id: string;
  name: string;
  path: string | null;
  version: string | null;
  source: string;
  available: boolean;
  required: boolean;
  note: string;
}

export interface ToolsStatus {
  ffmpeg: ToolInfo;
  ffprobe: ToolInfo;
  sevenZip: ToolInfo;
  realesrgan: ToolInfo;
  waifu2x: ToolInfo;
  ghostscript: ToolInfo;
  qpdf: ToolInfo;
  toolsDir: string;
  encoders: string[];
  portable: boolean;
}

export interface MediaInfo {
  path: string;
  name: string;
  kind: "image" | "video";
  width: number;
  height: number;
  duration: number;
  bytes: number;
  preview: string;
}

export interface UpscaleResult {
  output: string;
  engine: string;
  width: number;
  height: number;
  bytes: number;
  elapsedMs: number;
  beforePreview: string;
  afterPreview: string;
  note: string;
}

export interface UpscaleProgress {
  phase: string;
  percent: number;
  message: string;
}

export interface CodecSupport {
  h264: boolean;
  hevc: boolean;
  av1: boolean;
}

export interface EncoderSupport {
  vendor: string;
  nvenc: CodecSupport;
  amf: CodecSupport;
  qsv: CodecSupport;
  recommended: string;
  note: string;
}

export interface Bootstrap {
  version: string;
  settings: Settings;
  system: SystemInfo;
  tools: ToolsStatus;
  paths: Record<string, string>;
  settingsPath: string;
}

export interface KindCount {
  kind: string;
  files: number;
  bytes: number;
}

export interface ScanResult {
  files: number;
  bytes: number;
  counts: KindCount[];
  skipped: number;
  source: string;
  destination: string;
}

export interface KindStat {
  files: number;
  inBytes: number;
  outBytes: number;
}

export interface JobStats {
  totalFiles: number;
  processed: number;
  compressed: number;
  copied: number;
  failed: number;
  inBytes: number;
  outBytes: number;
  savedBytes: number;
  ratio: number;
  elapsedMs: number;
  cancelled: boolean;
  byKind: Record<string, KindStat>;
}

export interface Outcome {
  rel: string;
  kind: string;
  status: string;
  inBytes: number;
  outBytes: number;
  savedBytes: number;
  ratio: number;
  outPath: string;
  method: string;
  note: string;
  error: string;
  elapsedMs: number;
}

export interface ToolProgress {
  id: string;
  phase: string;
  downloaded: number;
  total: number;
  percent: number;
  message: string;
}

export interface JobStatus {
  running: boolean;
  stats: JobStats | null;
}

export interface LogLine {
  id: number;
  text: string;
  level: "info" | "dim" | "ok" | "warn" | "error" | "accent";
  time: string;
}

export const KIND_LABELS: Record<string, string> = {
  image: "Images",
  video: "Video",
  gif: "GIF",
  audio: "Audio",
  pdf: "PDF",
  office: "Office",
  archive: "Archive",
  text: "Text",
  svg: "SVG",
  font: "Font",
  lossless: "Lossless",
  copy: "Copied",
};

export const KIND_CHIPS = [
  "image",
  "video",
  "gif",
  "audio",
  "pdf",
  "office",
  "archive",
  "text",
  "svg",
  "font",
  "lossless",
];

export const PRESETS: Record<string, Partial<Settings>> = {
  "Smart": {
    lossy: 55,
    smartMode: true,
    smartMaxEncodes: 14,
    smartScaleSearch: true,
    smartContentAnalysis: true,
    smartGifRace: true,
    smartAudioGuard: true,
    imageFormat: "auto",
    convertPngToJpeg: false,
    gifMode: "auto",
    videoCodec: "auto",
    encoderPrefer: "quality",
    videoResolution: "keep",
    videoMaxFps: "keep",
    videoScalePct: 100,
    videoDenoise: false,
    video10bit: true,
    videoScaleSearch: "auto",
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    verify: true,
  },
  "Archive": {
    lossy: 82,
    smartMode: false,
    imageFormat: "avif",
    convertPngToJpeg: true,
    gifMode: "mp4",
    videoCodec: "h265",
    videoResolution: "1080p",
    videoMaxFps: "30",
    video10bit: true,
    videoScaleSearch: "auto",
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    imageMaxMp: 4,
  },
  "Web": {
    lossy: 65,
    smartMode: false,
    imageFormat: "webp",
    convertPngToJpeg: false,
    gifMode: "mp4",
    videoCodec: "h264",
    videoResolution: "1080p",
    videoMaxFps: "30",
    videoAudioCodec: "aac",
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
  },
  "Balanced": {
    lossy: 50,
    smartMode: false,
    imageFormat: "auto",
    convertPngToJpeg: false,
    gifMode: "auto",
    videoCodec: "auto",
    videoResolution: "keep",
    videoMaxFps: "keep",
    videoScalePct: 100,
    videoDenoise: false,
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    video10bit: true,
  },
  "Quality": {
    lossy: 22,
    smartMode: false,
    imageFormat: "keep",
    convertPngToJpeg: false,
    gifMode: "gif",
    videoCodec: "h264",
    videoResolution: "keep",
    videoMaxFps: "keep",
    pdfMode: "text",
    archiveMode: "outer",
    textMode: "keep",
    deepImage: false,
    video10bit: false,
    videoScaleSearch: "off",
  },
  "Maximum": {
    lossy: 95,
    smartMode: false,
    imageFormat: "avif",
    convertPngToJpeg: true,
    gifMode: "mp4",
    videoCodec: "h265",
    videoResolution: "720p",
    videoMaxFps: "24",
    imageMaxMp: 2,
    videoScalePct: 80,
    videoDenoise: true,
    video10bit: true,
    videoScaleSearch: "on",
    videoTwoPass: "on",
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    encoderPrefer: "quality",
    verify: true,
  },
  "Lossless Maximum": {
    lossy: 0,
    smartMode: false,
    deepImage: true,
    imageFormat: "keep",
    convertPngToJpeg: false,
    convertBmpToJpeg: false,
    gifMode: "gif",
    videoCodec: "auto",
    videoResolution: "keep",
    videoMaxFps: "keep",
    videoScalePct: 100,
    videoDenoise: false,
    video10bit: true,
    videoScaleSearch: "off",
    videoTwoPass: "auto",
    videoTune: "auto",
    pdfMode: "text",
    archiveMode: "outer",
    textMode: "keep",
    encoderPrefer: "quality",
    verify: true,
  },
  "Lossless Balanced": {
    lossy: 0,
    smartMode: false,
    deepImage: false,
    imageFormat: "auto",
    convertPngToJpeg: false,
    gifMode: "auto",
    videoCodec: "auto",
    videoResolution: "keep",
    videoMaxFps: "keep",
    videoScalePct: 100,
    videoDenoise: false,
    video10bit: true,
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    encoderPrefer: "balanced",
    verify: true,
  },
  "Lossless Fast": {
    lossy: 0,
    smartMode: false,
    deepImage: false,
    imageFormat: "auto",
    convertPngToJpeg: false,
    convertBmpToJpeg: false,
    gifMode: "gif",
    videoCodec: "h264",
    videoResolution: "keep",
    videoMaxFps: "keep",
    videoScalePct: 100,
    videoDenoise: false,
    video10bit: false,
    pdfMode: "max",
    archiveMode: "max",
    textMode: "auto",
    encoderPrefer: "speed",
    verify: false,
  },
};

export interface FileResult {
  rel: string;
  kind: string;
  status: string;
  inBytes: number;
  outBytes: number;
  ratio: number;
  method?: string;
  note?: string;
  error?: string;
}

export interface HistoryEntry {
  finishedAt: number;
  src: string;
  dst: string;
  stats: JobStats;
}

export interface JobEvent {
  event: "log" | "progress" | "file" | "stats" | "done" | "scan";
  message?: string;
  level?: LogLine["level"];
  overall?: number;
  fileFraction?: number;
  file?: string;
  done?: number;
  total?: number;
  files?: number;
  bytes?: number;
  counts?: Record<string, number>;
  stats?: JobStats;
  rel?: string;
  kind?: string;
  status?: string;
  inBytes?: number;
  outBytes?: number;
  ratio?: number;
  method?: string;
  note?: string;
  error?: string;
}

export function formatBytes(bytes: number, digits = 1): string {
  if (!bytes || bytes < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit === 0 ? `${Math.round(value)} B` : `${value.toFixed(digits)} ${units[unit]}`;
}

export function formatDuration(ms: number, lang = "en"): string {
  if (!ms || ms < 0) return tn("{s} s", lang, { s: 0 });
  const s = Math.round(ms / 1000);
  if (s < 60) return tn("{s} s", lang, { s });
  const m = Math.floor(s / 60);
  const rest = s % 60;
  if (m < 60) return tn("{m} min {s} s", lang, { m, s: rest });
  const h = Math.floor(m / 60);
  return tn("{h} h {m} min", lang, { h, m: m % 60 });
}

export type PlanValue = { label: string; value: string };

export function planValues(lossy: number): PlanValue[] {
  const factor = Math.min(100, Math.max(0, lossy)) / 100;
  const steps = [320, 256, 224, 192, 160, 128, 112, 96, 80, 64, 56, 48, 40, 32];
  const rawAudio = 256 - 188 * factor;
  const audio = steps.reduce((a, b) => (Math.abs(a - rawAudio) < Math.abs(b - rawAudio) ? a : b));
  return [
    { label: "Image", value: `q${Math.round(96 - 84 * factor)}` },
    { label: "Video", value: `CRF ${Math.round(15 + 25 * Math.pow(factor, 1.15))}` },
    { label: "Audio", value: `${audio}k` },
    { label: "GIF", value: `${Math.max(4, Math.round(20 - 14 * factor))}fps ${Math.max(160, Math.round((640 - 320 * factor) / 2) * 2)}px` },
    { label: "PDF", value: `${Math.max(60, Math.round((200 - 128 * factor) / 2) * 2)}dpi` },
  ];
}

export function kindLabel(kind: string, lang = "en"): string {
  return t(KIND_LABELS[kind] ?? kind, lang);
}
