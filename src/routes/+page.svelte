<script lang="ts">
  import { onMount } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    isPermissionGranted,
    requestPermission,
    sendNotification,
  } from "@tauri-apps/plugin-notification";
  import Icon from "$lib/components/Icon.svelte";
  import Toggle from "$lib/components/Toggle.svelte";
  import Slider from "$lib/components/Slider.svelte";
  import Accordion from "$lib/components/Accordion.svelte";
  import SettingsView from "$lib/views/Settings.svelte";
  import Upscaling from "$lib/views/Upscaling.svelte";
  import { t, tn, localizeLog } from "$lib/i18n";
  import { currentTip, tip, DEFAULT_TIP } from "$lib/tip";
  import type {
    Bootstrap,
    FileResult,
    HistoryEntry,
    JobEvent,
    JobStats,
    JobStatus,
    LogLine,
    ScanResult,
    Settings as AppSettings,
    SystemInfo,
    ToolsStatus,
  } from "$lib/types";
  import {
    KIND_CHIPS,
    PRESETS,
    formatBytes,
    formatDuration,
    kindLabel,
    planValues,
  } from "$lib/types";

  let app = $state<Bootstrap | null>(null);
  let settings = $state<AppSettings>({} as AppSettings);
  let system = $state<SystemInfo | null>(null);
  let tools = $state<ToolsStatus | null>(null);

  let loading = $state(true);
  let loadError = $state("");
  let tab = $state<"home" | "upscaling" | "settings">("home");
  let settingsSection = $state("general");
  let upscaleDrop = $state<{ path: string; seq: number } | null>(null);
  let upscaleDropSeq = 0;

  let src = $state("");
  let dst = $state("");
  let scanning = $state(false);
  let scanResult = $state<ScanResult | null>(null);

  let running = $state(false);
  let startedAt = $state<number | null>(null);
  let tick = $state(Date.now());
  let progress = $state(0);
  let currentFile = $state("");
  let done = $state(0);
  let total = $state(0);
  let stats = $state<JobStats | null>(null);
  let lastStats = $state<JobStats | null>(null);

  let results = $state<FileResult[]>([]);
  let resultFilter = $state<"all" | "compressed" | "copied" | "failed">("all");
  let history = $state<HistoryEntry[]>([]);

  const filteredResults = $derived(
    resultFilter === "all" ? results : results.filter((item) => item.status === resultFilter),
  );
  const failedResults = $derived(results.filter((item) => item.status === "failed"));

  const LOSSLESS_PRESETS = new Set([
    "Lossless Maximum",
    "Lossless Balanced",
    "Lossless Fast",
  ]);
  const PRESET_DESC: Record<string, string> = {
    "Lossless Maximum":
      "Deep analysis on, original format, lossless GIF, video CRF 15 (10-bit), quality-first encoder",
    "Lossless Balanced":
      "Automatic format, automatic GIF, video CRF 15 (10-bit), balanced encoder",
    "Lossless Fast":
      "Automatic format, lossless GIF, video H.264 CRF 15 (8-bit), speed-first encoder, verification off",
  };
  const visiblePresets = $derived(
    Object.keys(PRESETS).filter((p) =>
      settings.losslessMode ? LOSSLESS_PRESETS.has(p) : !LOSSLESS_PRESETS.has(p),
    ),
  );
  const activePreset = $derived(
    Object.keys(PRESETS).find((name) =>
      Object.entries(PRESETS[name] as Record<string, unknown>).every(
        ([key, value]) => (settings as unknown as Record<string, unknown>)[key] === value,
      ),
    ) ?? "Custom",
  );

  let logs = $state<LogLine[]>([]);
  let logSeq = 0;
  let logBox = $state<HTMLDivElement | undefined>(undefined);

  let toastMessage = $state("");
  let toastKind = $state<"ok" | "err">("ok");
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let hydrated = $state(false);
  let appliedZoom = 0;
  let appliedTheme = "";

  $effect(() => {
    const theme = settings.theme === "light" ? "light" : "dark";
    document.documentElement.dataset.theme = theme;
    document.documentElement.lang = settings.language === "tr" ? "tr" : "en";
    const scale = Math.min(1.6, Math.max(0.8, Number(settings.fontScale) || 1));
    if (!isTauri()) return;
    if (scale !== appliedZoom) {
      appliedZoom = scale;
      void getCurrentWebview().setZoom(scale).catch(() => {});
    }
    if (theme !== appliedTheme) {
      appliedTheme = theme;
      void getCurrentWindow().setTheme(theme).catch(() => {});
    }
  });

  $effect(() => {
    if (!hydrated) return;
    const snapshot = $state.snapshot(settings);
    JSON.stringify(snapshot);
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void invoke("save_settings", { settings: snapshot }).catch((err) => showToast(String(err), "err"));
    }, 700);
  });

  $effect(() => {
    if (!running) return;
    const id = setInterval(() => (tick = Date.now()), 1000);
    return () => clearInterval(id);
  });

  $effect(() => {
    logs.length;
    if (logBox) logBox.scrollTop = logBox.scrollHeight;
  });

  const elapsedMs = $derived(
    running && startedAt
      ? tick - startedAt
      : (stats?.elapsedMs ?? lastStats?.elapsedMs ?? 0),
  );

  const savedPercent = $derived(
    stats && stats.inBytes > 0 ? (stats.ratio * 100).toFixed(1) : "0.0",
  );

  const gpuLabel = $derived(
    system?.gpus.find((gpu) => !gpu.software)?.name ?? t("No GPU", settings.language),
  );

  const KIND_ICONS: Record<string, string> = {
    image: "type-image",
    video: "type-video",
    gif: "type-gif",
    audio: "type-audio",
    pdf: "type-pdf",
    office: "type-office",
    archive: "type-archive",
    text: "type-text",
    svg: "type-svg",
    font: "type-font",
    lossless: "shield",
    copy: "check",
  };

  function kindIcon(kind: string): string {
    return KIND_ICONS[kind] ?? "type-other";
  }

  function logTag(level: LogLine["level"]): string {
    const key =
      level === "warn"
        ? "WARN"
        : level === "error"
          ? "ERR"
          : level === "ok"
            ? "OK"
            : level === "accent"
              ? "JOB"
              : "INFO";
    return t(key, settings.language);
  }

  function logTagClass(level: LogLine["level"]): string {
    if (level === "warn") return "wa";
    if (level === "error") return "er";
    if (level === "ok") return "ok";
    if (level === "accent") return "ac";
    return "if";
  }

  onMount(() => {
    const unJob = listen<JobEvent>("job-event", (event) => handleJobEvent(event.payload));
    const unDrop = getCurrentWindow().onDragDropEvent((event) => {
      if (event.payload.type !== "drop") return;
      const dropped = event.payload.paths?.[0];
      if (!dropped) return;
      if (tab === "upscaling") {
        upscaleDrop = { path: dropped, seq: ++upscaleDropSeq };
        return;
      }
      void invoke<string>("resolve_source", { path: dropped })
        .then((folder) => applySource(folder))
        .catch((err) => showToast(String(err), "err"));
    });
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "F5") return;
      event.preventDefault();
      if (event.repeat || tab !== "home") return;
      void startOrCancel();
    };
    window.addEventListener("keydown", onKey);
    void load();
    return () => {
      void unJob.then((un) => un());
      void unDrop.then((un) => un());
      window.removeEventListener("keydown", onKey);
      clearTimeout(saveTimer);
      clearTimeout(toastTimer);
    };
  });

  function applySource(folder: string) {
    src = folder;
    if (!dst.trim()) dst = suggestDestination(folder);
    scanResult = null;
  }

  async function loadHistory() {
    try {
      history = await invoke<HistoryEntry[]>("job_history", { limit: 8 });
    } catch {
      history = [];
    }
  }

  function applyHistory(entry: HistoryEntry) {
    src = entry.src;
    dst = entry.dst;
    scanResult = null;
  }

  async function clearHistory() {
    try {
      await invoke("clear_history");
      history = [];
    } catch (err) {
      showToast(String(err), "err");
    }
  }

  function statusLabel(status: string): string {
    const labels: Record<string, string> = {
      compressed: "Compressed",
      copied: "Copied",
      failed: "Failed",
      skipped: "Skipped",
      cancelled: "Cancelled",
    };
    return t(labels[status] ?? status, settings.language);
  }

  function historyDate(ms: number): string {
    return new Date(ms).toLocaleString(settings.language === "tr" ? "tr-TR" : "en-GB", {
      dateStyle: "short",
      timeStyle: "short",
    });
  }

  async function load() {
    loading = true;
    loadError = "";
    try {
      const data = await invoke<Bootstrap>("bootstrap");
      app = data;
      settings = data.settings;
      system = data.system;
      tools = data.tools;
      const status = await invoke<JobStatus>("job_status");
      running = status.running;
      if (status.stats) {
        stats = status.stats;
        lastStats = status.stats;
      }
      if (status.running) {
        startedAt = Date.now();
        if (status.stats?.totalFiles) {
          done = status.stats.processed;
          total = status.stats.totalFiles;
          progress = Math.min(1, status.stats.processed / status.stats.totalFiles);
        }
        pushLog(t("An ongoing job was detected; tracking progress.", settings.language), "warn");
      }
      await loadHistory();
      hydrated = true;
    } catch (err) {
      loadError = String(err);
    } finally {
      loading = false;
    }
  }

  function showToast(message: string, kind: "ok" | "err" = "ok") {
    toastMessage = localizeLog(message, settings.language);
    toastKind = kind;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toastMessage = ""), 4200);
  }

  async function notifyFinished(result: JobStats | null) {
    if (!settings.notifyDone || !result) return;
    let shown = false;
    try {
      let granted = await isPermissionGranted();
      if (!granted) granted = (await requestPermission()) === "granted";
      if (granted) {
        const body = result.cancelled
          ? t("Job cancelled.", settings.language)
          : tn("{in} → {out} · {saved} saved", settings.language, {
              in: formatBytes(result.inBytes),
              out: formatBytes(result.outBytes),
              saved: formatBytes(result.savedBytes),
            });
        sendNotification({
          title: result.cancelled
            ? t("ShrinkWise · Job cancelled", settings.language)
            : t("ShrinkWise · Compression complete", settings.language),
          body,
        });
        shown = true;
      }
    } catch {
      // Falls back to the taskbar attention request below.
    }
    if (!shown) {
      try {
        await getCurrentWindow().requestUserAttention(1);
      } catch {
        // ignore
      }
    }
  }

  function pushLog(message: string, level: LogLine["level"] = "info") {
    if (!message) return;
    const line: LogLine = {
      id: ++logSeq,
      text: localizeLog(message, settings.language),
      level,
      time: new Date().toLocaleTimeString(settings.language === "tr" ? "tr-TR" : "en-GB", {
        hour12: false,
      }),
    };
    logs = [...logs.slice(-799), line];
  }

  function handleJobEvent(event: JobEvent) {
    switch (event.event) {
      case "log":
        pushLog(event.message ?? "", event.level ?? "info");
        break;
      case "stats":
        if (event.stats) {
          stats = event.stats;
          done = event.stats.processed;
          total = event.stats.totalFiles;
        }
        break;
      case "progress":
        if (typeof event.overall === "number") progress = Math.max(progress, Math.max(0, Math.min(1, event.overall)));
        if (event.file) currentFile = event.file;
        if (typeof event.done === "number") done = event.done;
        if (typeof event.total === "number") total = event.total;
        break;
      case "scan":
        if (typeof event.files === "number" && typeof event.bytes === "number") {
          const prev = scanResult && scanResult.source === src ? scanResult : null;
          const prevBytes = new Map(prev?.counts.map((c) => [c.kind, c.bytes]) ?? []);
          scanResult = {
            files: event.files,
            bytes: event.bytes,
            counts: Object.entries(event.counts ?? {}).map(([kind, files]) => ({
              kind,
              files,
              bytes: prevBytes.get(kind) ?? 0,
            })),
            skipped: prev?.skipped ?? 0,
            source: src,
            destination: dst,
          };
        }
        break;
      case "file":
        if (event.rel) {
          const item: FileResult = {
            rel: event.rel,
            kind: event.kind ?? "",
            status: event.status ?? "",
            inBytes: event.inBytes ?? 0,
            outBytes: event.outBytes ?? 0,
            ratio: event.ratio ?? 0,
            method: event.method,
            note: event.note,
            error: event.error,
          };
          results = [...results.filter((entry) => entry.rel !== item.rel), item].slice(-5000);
        }
        break;
      case "done":
        running = false;
        startedAt = null;
        progress = 1;
        currentFile = "";
        if (event.stats) {
          stats = event.stats;
          lastStats = event.stats;
          done = event.stats.processed;
          total = event.stats.totalFiles;
        }
        showToast(
          event.stats?.cancelled
            ? t("Job cancelled", settings.language)
            : t("Compression complete", settings.language),
          event.stats?.cancelled ? "err" : "ok",
        );
        void notifyFinished(event.stats ?? null);
        void loadHistory();
        break;
    }
  }

  function suggestDestination(source: string): string {
    return `${source.replace(/[\\/]+$/, "")}-small`;
  }

  async function pickSource() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: t("Select source folder", settings.language),
    });
    if (typeof picked === "string") {
      src = picked;
      if (!dst.trim()) dst = suggestDestination(picked);
      scanResult = null;
    }
  }

  async function pickDestination() {
    const picked = await open({
      directory: true,
      multiple: false,
      title: t("Select destination folder", settings.language),
    });
    if (typeof picked === "string") {
      dst = picked;
      scanResult = null;
    }
  }

  function swapPaths() {
    if (!src.trim() || !dst.trim()) {
      showToast(t("Select source and destination folders", settings.language), "err");
      return;
    }
    const oldSrc = src;
    src = dst;
    dst = oldSrc;
    scanResult = null;
  }

  async function scan() {
    if (!src.trim()) {
      showToast(t("Select a source folder first", settings.language), "err");
      return;
    }
    scanning = true;
    try {
      const result = await invoke<ScanResult>("scan_source", {
        src,
        dst: dst.trim() || src,
        settings: $state.snapshot(settings),
      });
      scanResult = result;
      showToast(
        tn("Scanned {n} files · {size}", settings.language, {
          n: result.files,
          size: formatBytes(result.bytes),
        }),
        "ok",
      );
    } catch (err) {
      showToast(String(err), "err");
    } finally {
      scanning = false;
    }
  }

  async function startJob(only?: string[]) {
    if (running) return;
    if (!src.trim() || !dst.trim()) {
      showToast(t("Select source and destination folders", settings.language), "err");
      return;
    }
    try {
      const response = await invoke<{ started: boolean; message: string }>("start_job", {
        src,
        dst,
        settings: $state.snapshot(settings),
        only: only && only.length ? only : null,
      });
      if (!response.started) {
        showToast(response.message, "err");
        return;
      }
      logs = [];
      stats = null;
      lastStats = null;
      results = [];
      progress = 0;
      done = 0;
      total = 0;
      currentFile = "";
      startedAt = Date.now();
      tick = Date.now();
      running = true;
      pushLog(tn("Started: {msg}", settings.language, { msg: response.message }), "accent");
    } catch (err) {
      showToast(String(err), "err");
    }
  }

  function retryFailed() {
    if (!failedResults.length) return;
    void startJob(failedResults.map((item) => item.rel));
  }

  async function cancelJob() {
    try {
      const accepted = await invoke<boolean>("cancel_job");
      if (accepted) {
        pushLog(
          t("Cancellation requested; waiting for running files to finish…", settings.language),
          "warn",
        );
      } else {
        showToast(t("No running job", settings.language), "err");
      }
    } catch (err) {
      showToast(String(err), "err");
    }
  }

  async function startOrCancel() {
    if (running) await cancelJob();
    else await startJob();
  }

  async function exportReport(format: "csv" | "json") {
    if (!lastStats) {
      showToast(t("Complete a job first", settings.language), "err");
      return;
    }
    try {
      const path = await save({
        title: tn("ShrinkWise report ({format})", settings.language, {
          format: format.toUpperCase(),
        }),
        defaultPath: `ShrinkWise-report.${format}`,
        filters: [
          {
            name: format.toUpperCase(),
            extensions: [format],
          },
        ],
      });
      if (!path) return;
      const written = await invoke<string>("export_report", { path, format });
      showToast(tn("Report saved: {path}", settings.language, { path: written }), "ok");
    } catch (err) {
      showToast(String(err), "err");
    }
  }

  async function openDestination() {
    if (!dst.trim()) {
      showToast(t("Select a destination folder first", settings.language), "err");
      return;
    }
    try {
      await openPath(dst);
    } catch (err) {
      showToast(String(err), "err");
    }
  }

  function applyPreset(name: string) {
    Object.assign(settings, PRESETS[name]);
    showToast(
      tn("{name} profile applied", settings.language, { name: t(name, settings.language) }),
      "ok",
    );
  }

  function toggleKind(kind: string) {
    if (!settings.onlyKinds.length) {
      settings.onlyKinds = [kind];
      return;
    }
    const set = new Set(settings.onlyKinds);
    if (set.has(kind)) set.delete(kind);
    else set.add(kind);
    settings.onlyKinds = set.size === KIND_CHIPS.length ? [] : [...set];
  }

  function isKindOn(kind: string): boolean {
    return !settings.onlyKinds.length || settings.onlyKinds.includes(kind);
  }

  function goSettings(section = "general") {
    settingsSection = section;
    tab = "settings";
  }
</script>

<div class="win">
  <header class="hdr">
    <div class="brand">
      <svg width="34" height="34" viewBox="0 0 1024 1024">
        <path d="M904 560V120H120V904H560" fill="none" stroke="var(--ink)" stroke-width="112" />
        <rect x="592" y="592" width="256" height="256" fill="var(--accent)" />
      </svg>
      <b>ShrinkWise</b>
      <span class="ver">v{app?.version ?? "3.0"}</span>
    </div>
    <div class="tag">
      {t("One setting. Every file. Smallest size.", settings.language)}<br />
      <span>{t("video · audio · GIF · image · PDF · office · archive · text · font", settings.language)}</span>
    </div>
    <div class="sp"></div>
    <div class="seg">
      <button class:active={tab === "home"} aria-pressed={tab === "home"} onclick={() => (tab = "home")}><Icon name="package" size={15} /> {t("Compression", settings.language)}</button>
      <button class:active={tab === "upscaling"} aria-pressed={tab === "upscaling"} onclick={() => (tab = "upscaling")}><Icon name="sparkles" size={15} /> {t("Upscaling", settings.language)}</button>
    </div>
    <div class="stat"><i class={system?.gpus?.some((g) => !g.software) ? "ok" : ""}></i><b>GPU</b><span class="stat-name">{gpuLabel}</span></div>
    <div class="stat"><i class={tools?.ffmpeg.available ? "ok" : ""}></i><b>FFmpeg</b> {tools?.ffmpeg.available ? t("ready", settings.language) : t("missing", settings.language)}</div>
    {#if tab === "home"}
      <div class="seg">
        <button class:active={settings.simpleMode} aria-pressed={settings.simpleMode} onclick={() => (settings.simpleMode = true)}>{t("Simple", settings.language)}</button>
        <button class:active={!settings.simpleMode} aria-pressed={!settings.simpleMode} onclick={() => (settings.simpleMode = false)}>{t("Advanced", settings.language)}</button>
      </div>
    {/if}
    <button class="icon-btn" onclick={() => (settings.theme = settings.theme === "dark" ? "light" : "dark")} title={t("Toggle theme", settings.language)} aria-label={t("Toggle theme", settings.language)}>
      <Icon name={settings?.theme === "light" ? "sun" : "moon"} size={17} />
    </button>
    <button class="icon-btn" onclick={() => goSettings()} title={t("Settings", settings.language)} aria-label={t("Settings", settings.language)}>
      <Icon name="settings" size={17} />
    </button>
  </header>

  {#if loading}
    <div class="empty-state">
      <div class="spinner"></div>
      <div>{t("Detecting hardware and tools…", settings.language)}</div>
    </div>
  {:else if loadError || !app}
    <div class="empty-state">
      <Icon name="alert" size={34} />
      <div>{t("Application could not start.", settings.language)}</div>
      <div class="muted small mono" style="max-width:640px;text-align:center">{loadError}</div>
      <button class="btn primary" onclick={load}><Icon name="refresh" size={15} /> {t("Retry", settings.language)}</button>
    </div>
  {:else if tab === "settings"}
    <SettingsView
      bind:settings={settings}
      {system}
      {tools}
      version={app.version}
      settingsPath={app.settingsPath}
      paths={app.paths}
      bind:section={settingsSection}
      onToolsChanged={(next) => (tools = next)}
      onSystemChanged={(next) => (system = next)}
      toast={showToast}
      onBack={() => (tab = "home")}
    />
  {:else if tab === "upscaling"}
    <div class="main" style="grid-template-columns: 1fr">
      <div class="sheet">
        <Upscaling
          lang={settings.language}
          {settings}
          {tools}
          drop={upscaleDrop}
          onOpenTools={() => goSettings("tools")}
        />
      </div>
    </div>
  {:else}
    <div class="main">
      <div class="sheet">
        <div class="sec">
          <div class="n">01</div>
          <div class="sec-body">
            <div class="hd">
              <h2>{t("Folders", settings.language)}</h2>
              <div class="d">
                {t("Every file in the source folder is written to the destination, keeping the folder structure.", settings.language)}
              </div>
            </div>

            <div class="field">
              <span class="lb">{t("Source", settings.language)}</span>
              <input class="in v" placeholder={t("Folder to compress…", settings.language)} aria-label={t("Source", settings.language)} spellcheck="false" bind:value={src} />
              <button class="btn" onclick={pickSource} use:tip={t("Select source folder", settings.language)}>
                <Icon name="folder-source" size={16} /> {t("Browse", settings.language)}
              </button>
            </div>

            <div class="field">
              <span class="lb">{t("Destination", settings.language)}</span>
              <input class="in v" placeholder={t("Output folder…", settings.language)} aria-label={t("Destination", settings.language)} spellcheck="false" bind:value={dst} />
              <button class="btn" onclick={pickDestination} use:tip={t("Select destination folder", settings.language)}>
                <Icon name="folder-target" size={16} /> {t("Browse", settings.language)}
              </button>
            </div>

            <div class="field scan-row">
              <span class="lb">{t("Scan", settings.language)}</span>
              <button
                class="btn sm"
                disabled={scanning}
                onclick={scan}
                use:tip={t("Counts files by type and calculates total size", settings.language)}
              >
                {#if scanning}
                  <span class="spinner sm"></span>
                {:else}
                  <Icon name="scan" size={14} />
                {/if}
                {t("Scan", settings.language)}
              </button>
              <button class="btn sm" onclick={swapPaths} use:tip={t("Swap source and destination", settings.language)}>
                <Icon name="swap" size={14} /> {t("Swap", settings.language)}
              </button>
              <span class="muted small scan-summary">
                {#if scanResult}
                  {scanResult.files} {t("files", settings.language)} · {formatBytes(scanResult.bytes)}
                  {#if scanResult.skipped > 0}
                    · {scanResult.skipped} {t("skipped", settings.language)}
                  {/if}
                {:else}
                  {t("Tip: you can drag a folder onto the window.", settings.language)}
                {/if}
              </span>
            </div>

            {#if scanResult && scanResult.counts.length}
              <div class="kv scan-kv">
                {#each scanResult.counts as count}
                  <div>
                    <span>{kindLabel(count.kind, settings.language)}</span>
                    <b>{count.files}</b>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>

        <div class="sec">
          <div class="n">02</div>
          <div class="sec-body">
            <div class="hd">
              <h2>{t("Compression ratio", settings.language)}</h2>
              <div class="d">
                {t("A single slider that scales image quality, video CRF, audio bitrate and GIF settings together.", settings.language)}
              </div>
            </div>

            <Slider
              label={t("Lossy", settings.language)}
              bind:value={settings.lossy}
              min={0}
              max={100}
              suffix="%"
              low={t("higher quality", settings.language)}
              high={t("smallest file", settings.language)}
            />

            <div class="pre">
              {#each visiblePresets as preset}
                <button
                  class="pre-btn"
                  class:on={activePreset === preset}
                  aria-pressed={activePreset === preset}
                  onclick={() => applyPreset(preset)}
                  use:tip={t(
                    PRESET_DESC[preset] ??
                      tn("Apply {name} profile", settings.language, {
                        name: t(preset, settings.language),
                      }),
                    settings.language,
                  )}
                >
                  <b>%{PRESETS[preset].lossy ?? 0}</b>
                  <span>{t(preset, settings.language)}</span>
                </button>
              {/each}
            </div>

            <div
              class="row lossless-row"
              use:tip={t("When on, only lossless (lossy=0) presets are shown; when off, normal presets", settings.language)}
            >
              <b>{t("Lossless mode", settings.language)}</b>
              <button
                class="sw"
                class:on={settings.losslessMode}
                aria-pressed={settings.losslessMode}
                aria-label={t("Lossless mode", settings.language)}
                onclick={() => {
                  const next = !settings.losslessMode;
                  settings.losslessMode = next;
                  if (next) settings.lossy = 0;
                }}
              ></button>
            </div>

            <div class="plan">
              <div class="plan-head">{t("Automatic plan", settings.language)}</div>
              <div class="plan-grid">
                {#each planValues(settings.lossy) as item}
                  <div><span>{t(item.label, settings.language)}</span><b>{item.value}</b></div>
                {/each}
              </div>
            </div>

            {#if settings.losslessMode}
              <div class="note">
                <p>
                  {t("All run lossless (lossy=0); the difference is how much power goes into compression and the quality balance. Hover a preset to see details in the info bar below.", settings.language)}
                </p>
                <div class="kv"><span class="k">{t("Lossless Maximum", settings.language)}</span><span class="v">{t("Deep analysis on, original format, lossless GIF, video CRF 15 (10-bit), quality-first encoder", settings.language)}</span></div>
                <div class="kv"><span class="k">{t("Lossless Balanced", settings.language)}</span><span class="v">{t("Automatic format, automatic GIF, video CRF 15 (10-bit), balanced encoder", settings.language)}</span></div>
                <div class="kv"><span class="k">{t("Lossless Fast", settings.language)}</span><span class="v">{t("Automatic format, lossless GIF, video H.264 CRF 15 (8-bit), speed-first encoder, verification off", settings.language)}</span></div>
              </div>
            {/if}
          </div>
        </div>

        <div class="sec">
          <div class="n">03</div>
          <div class="sec-body">
            <div class="hd">
              <h2>{t("File types", settings.language)}</h2>
              <div class="d">
                {t("If none are selected, all types are processed. Selected types are included.", settings.language)}
              </div>
            </div>

            <div class="tiles">
              {#each KIND_CHIPS as kind}
                <button
                  class="tile"
                  class:on={isKindOn(kind)}
                  aria-pressed={isKindOn(kind)}
                  onclick={() => toggleKind(kind)}
                  use:tip={tn("Process {name} files", settings.language, {
                    name: kindLabel(kind, settings.language),
                  })}
                >
                  <Icon name={kindIcon(kind)} size={21} />
                  <span>{kindLabel(kind, settings.language)}</span>
                </button>
              {/each}
              <button
                class="tile"
                class:on={!settings.onlyKinds.length}
                aria-pressed={!settings.onlyKinds.length}
                onclick={() => (settings.onlyKinds = [])}
                use:tip={t("If none are selected, all types are processed. Selected types are included.", settings.language)}
              >
                <Icon name="check" size={21} />
                <span>{t("All", settings.language)}</span>
              </button>
            </div>

            <div class="form-grid" style="margin-top:12px">
              <div class="form-item">
                <span class="lbl">{t("Image format", settings.language)}</span>
                <select class="input" bind:value={settings.imageFormat}>
                  <option value="auto">{t("Automatic (by source)", settings.language)}</option>
                  <option value="keep">{t("Keep original", settings.language)}</option>
                  <option value="avif">AVIF</option>
                  <option value="webp">WebP</option>
                  <option value="jpg">JPEG</option>
                </select>
              </div>
              <div class="form-item">
                <span class="lbl">{t("Video codec", settings.language)}</span>
                <select class="input" bind:value={settings.videoCodec}>
                  <option value="auto">{t("Automatic", settings.language)}</option>
                  <option value="h264">H.264</option>
                  <option value="h265">H.265 / HEVC</option>
                  <option value="av1">AV1</option>
                  <option value="vp9">VP9</option>
                </select>
              </div>
            </div>

            <Toggle
              label={t("Keep larger output", settings.language)}
              desc={t("Copy the original if compression does not shrink the file.", settings.language)}
              checked={settings.keepLarger}
              onchange={(value) => (settings.keepLarger = value)}
            />
            <Toggle
              label={t("Verify output", settings.language)}
              desc={t("Check that every output can be opened; report failures.", settings.language)}
              checked={settings.verify}
              onchange={(value) => (settings.verify = value)}
            />
          </div>
        </div>

        {#if !settings.simpleMode}
          <div class="sec">
            <div class="n">04</div>
            <div class="sec-body">
              <div class="hd">
                <h2>{t("Advanced", settings.language)}</h2>
                <div class="d">{t("Keep defaults unless needed.", settings.language)}</div>
              </div>

              <Accordion title={t("Image and GIF", settings.language)} subtitle={t("Quality, resolution, animation", settings.language)} icon="type-image">
                <div class="form-grid">
                  <div class="form-item">
                    <span class="lbl">{t("Max resolution (MP)", settings.language)}</span>
                    <input class="input" type="number" min="0" step="0.5" bind:value={settings.imageMaxMp} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Image quality (0 = automatic)", settings.language)}</span>
                    <input class="input" type="number" min="0" max="100" bind:value={settings.imageQuality} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("GIF processing", settings.language)}</span>
                    <select class="input" bind:value={settings.gifMode}>
                      <option value="auto">{t("Automatic", settings.language)}</option>
                      <option value="gif">{t("Optimize as GIF", settings.language)}</option>
                      <option value="webp">{t("Animated WebP", settings.language)}</option>
                      <option value="mp4">{t("Convert to MP4", settings.language)}</option>
                      <option value="keep">{t("Keep", settings.language)}</option>
                    </select>
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("GIF max FPS", settings.language)}</span>
                    <select class="input" bind:value={settings.gifMaxFps}>
                      <option value="auto">{t("Automatic", settings.language)}</option>
                      <option value="60">60</option>
                      <option value="30">30</option>
                      <option value="24">24</option>
                      <option value="15">15</option>
                      <option value="10">10</option>
                    </select>
                  </div>
                </div>
                <Toggle
                  label="PNG → JPEG"
                  desc={t("Produce smaller JPEG when there is no transparency.", settings.language)}
                  checked={settings.convertPngToJpeg}
                  onchange={(value) => (settings.convertPngToJpeg = value)}
                />
              </Accordion>

              <Accordion title={t("Video and audio", settings.language)} subtitle={t("Resolution, FPS, audio bitrate", settings.language)} icon="type-video">
                <div class="form-grid">
                  <div class="form-item">
                    <span class="lbl">{t("Resolution", settings.language)}</span>
                    <select class="input" bind:value={settings.videoResolution}>
                      <option value="keep">{t("Keep", settings.language)}</option>
                      <option value="2160p">2160p</option>
                      <option value="1440p">1440p</option>
                      <option value="1080p">1080p</option>
                      <option value="720p">720p</option>
                      <option value="480p">480p</option>
                    </select>
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Max FPS", settings.language)}</span>
                    <select class="input" bind:value={settings.videoMaxFps}>
                      <option value="keep">{t("Keep", settings.language)}</option>
                      <option value="60">60</option>
                      <option value="30">30</option>
                      <option value="24">24</option>
                    </select>
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Scale (%)", settings.language)}</span>
                    <input class="input" type="number" min="25" max="100" bind:value={settings.videoScalePct} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Audio codec", settings.language)}</span>
                    <select class="input" bind:value={settings.videoAudioCodec}>
                      <option value="auto">{t("Automatic", settings.language)}</option>
                      <option value="aac">AAC</option>
                      <option value="opus">Opus</option>
                      <option value="mp3">MP3</option>
                      <option value="copy">{t("Copy", settings.language)}</option>
                    </select>
                  </div>
                </div>
                <Toggle
                  label={t("GPU encoding", settings.language)}
                  desc={t("Use NVENC / AMF / QSV if available.", settings.language)}
                  checked={settings.gpu}
                  onchange={(value) => (settings.gpu = value)}
                />
                <Toggle
                  label={t("10-bit encoding", settings.language)}
                  desc={t("Reduces banding.", settings.language)}
                  checked={settings.video10bit}
                  onchange={(value) => (settings.video10bit = value)}
                />
                <Toggle
                  label={t("Denoise", settings.language)}
                  desc={t("Better compression for grainy videos.", settings.language)}
                  checked={settings.videoDenoise}
                  onchange={(value) => (settings.videoDenoise = value)}
                />
                <Toggle
                  label={t("Normalize audio", settings.language)}
                  desc={t("Balances audio level.", settings.language)}
                  checked={settings.audioNormalize}
                  onchange={(value) => (settings.audioNormalize = value)}
                />
              </Accordion>

              <Accordion title={t("Advanced filters", settings.language)} subtitle={t("Exclusion, size limit, worker count", settings.language)} icon="filter">
                <div class="form-grid">
                  <div class="form-item">
                    <span class="lbl">{t("Concurrent workers", settings.language)}</span>
                    <input class="input" type="number" min="1" max="32" bind:value={settings.workers} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Smallest file (KB)", settings.language)}</span>
                    <input class="input" type="number" min="0" bind:value={settings.minSizeKb} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Largest file (MB, 0 = unlimited)", settings.language)}</span>
                    <input class="input" type="number" min="0" bind:value={settings.limitMb} />
                  </div>
                  <div class="form-item">
                    <span class="lbl">{t("Dry run", settings.language)}</span>
                    <select class="input" bind:value={settings.dryRun}>
                      <option value={false}>{t("No, write for real", settings.language)}</option>
                      <option value={true}>{t("Yes, plan only", settings.language)}</option>
                    </select>
                  </div>
                  <div class="form-item wide">
                    <span class="lbl">{t("Patterns to exclude", settings.language)}</span>
                    <input class="input" placeholder="*.tmp, node_modules, .git" bind:value={settings.excludeGlobs} />
                  </div>
                </div>
                <div class="row">
                  <button class="btn sm" onclick={() => (settings.workers = system?.recommendedWorkers ?? settings.workers)}>
                    <Icon name="zap" size={13} /> {tn("Recommended workers: {n}", settings.language, {
                      n: system?.recommendedWorkers ?? "—",
                    })}
                  </button>
                  <button class="btn sm" onclick={() => goSettings("hardware")}>
                    <Icon name="cpu" size={13} /> {t("Hardware settings", settings.language)}
                  </button>
                </div>
              </Accordion>
            </div>
          </div>
        {/if}

        {#if results.length}
          <div class="sec">
            <div class="n"><Icon name="list" size={14} /></div>
            <div class="sec-body">
              <div class="hd">
                <h2>{t("Results", settings.language)}</h2>
                <div class="d">{tn("{n} files processed", settings.language, { n: results.length })}</div>
              </div>
              <div class="result-filters">
                {#each (["all", "compressed", "copied", "failed"] as const) as filter}
                  <button class="chip" class:active={resultFilter === filter} aria-pressed={resultFilter === filter} onclick={() => (resultFilter = filter)}>
                    {filter === "all" ? t("All", settings.language) : statusLabel(filter)}
                  </button>
                {/each}
                {#if failedResults.length}
                  <button class="btn sm" style="margin-left:auto" onclick={retryFailed} disabled={running}>
                    <Icon name="refresh" size={12} />
                    {tn("Retry failed ({n})", settings.language, { n: failedResults.length })}
                  </button>
                {/if}
              </div>
              <div class="results-table">
                {#each filteredResults.slice(0, 1000) as item (item.rel)}
                  <div class="result-row">
                    <span class="result-status {item.status}">{statusLabel(item.status)}</span>
                    <span class="result-rel" title={item.rel}>{item.rel}</span>
                    <span class="result-size">{formatBytes(item.inBytes)} → {formatBytes(item.outBytes)}</span>
                    <span class="result-ratio" class:ok={item.ratio > 0.005}>
                      {item.status === "failed" ? "—" : `−${Math.round(Math.max(0, item.ratio) * 100)}%`}
                    </span>
                  </div>
                {:else}
                  <div class="muted small" style="padding:8px 0">
                    {t("No files in this filter.", settings.language)}
                  </div>
                {/each}
              </div>
            </div>
          </div>
        {/if}
      </div>

      <div class="right-col">
        <div class="mon">
          <div class="mtop">
            <span class="cap">{t("Live progress", settings.language)}</span>
            <span class="cur">
              {#if running}
                {currentFile || t("Preparing…", settings.language)}
              {:else if lastStats}
                {tn("Last job: {in} → {out}", settings.language, {
                  in: formatBytes(lastStats.inBytes),
                  out: formatBytes(lastStats.outBytes),
                })}
              {:else}
                {t("Ready. Select a source and destination folder.", settings.language)}
              {/if}
            </span>
          </div>

          <div class="pct" class:idle={!running && !stats}>
            <b>{Math.round(progress * 100)}<small>%</small></b>
            <span class="cnt">{done}<em> / {total || "—"}</em></span>
          </div>

          <div class="bar">
            <i style={`width:${Math.max(progress * 100, running ? 2 : 0)}%`}></i>
          </div>

          <div class="sz">
            <span class="cap">{t("Size", settings.language)}</span>
            <div class="sr">
              <span class="lb">{t("Input", settings.language)}</span>
              <div class="b"><i style={`width:${stats?.inBytes ? 100 : 0}%`}></i></div>
              <span class="v">{stats ? formatBytes(stats.inBytes) : "—"}</span>
            </div>
            <div class="sr">
              <span class="lb">{t("Output", settings.language)}</span>
              <div class="b">
                <i
                  style={`width:${stats && stats.inBytes ? Math.min(100, (stats.outBytes / stats.inBytes) * 100) : 0}%`}
                ></i>
              </div>
              <span class="v">{stats ? formatBytes(stats.outBytes) : "—"}</span>
            </div>
          </div>

          <div class="stats">
            <div>
              <span class="lb">{t("Savings", settings.language)}</span>
              <b class:ok={!!stats && stats.ratio > 0} class:bad={!!stats && stats.ratio < 0}>{stats ? `${savedPercent}%` : "—"}</b>
            </div>
            <div>
              <span class="lb">{t("Duration", settings.language)}</span>
              <b>{formatDuration(elapsedMs, settings.language)}</b>
            </div>
          </div>

          <div class="lh">
            <span class="cap">{t("Live log", settings.language)}</span>
            <div class="sp"></div>
            <button class="gh" disabled={!lastStats} onclick={() => exportReport("csv")}>CSV</button>
            <button class="gh" disabled={!lastStats} onclick={() => exportReport("json")}>JSON</button>
            <button class="gh" disabled={!logs.length} onclick={() => (logs = [])}>
              <Icon name="trash" size={12} /> {t("Clear", settings.language)}
            </button>
          </div>

          <div class="log" bind:this={logBox}>
            {#each logs as line (line.id)}
              <div class="lg">
                <u class={logTagClass(line.level)}>{logTag(line.level)}</u>
                <span>{line.text}</span>
                <em>{line.time}</em>
              </div>
            {:else}
              <div class="lg msg">
                <u class="if">{logTag("info")}</u>
                <span>{t("No log yet. Use “Scan” to count files, F5 to start compression.", settings.language)}</span>
              </div>
            {/each}
          </div>
        </div>

        {#if history.length}
          <div class="hist">
            <div class="hist-head">
              <span class="cap">{t("Recent runs", settings.language)}</span>
              <button class="gh light" onclick={clearHistory}>
                <Icon name="trash" size={12} /> {t("Clear", settings.language)}
              </button>
            </div>
            <div class="history-list">
              {#each history.slice().reverse() as entry, index (entry.finishedAt + "-" + index)}
                <button class="history-row" onclick={() => applyHistory(entry)} title={entry.src}>
                  <span class="history-meta">
                    <span class="history-date">{historyDate(entry.finishedAt)}</span>
                    <span class="history-src">{entry.src}</span>
                  </span>
                  <span class="history-stat" class:ok={entry.stats.ratio > 0.005}>
                    {#if entry.stats.cancelled}
                      {t("Cancelled", settings.language)}
                    {:else}
                      −{Math.round(Math.max(0, entry.stats.ratio) * 100)}%
                    {/if}
                  </span>
                </button>
              {/each}
            </div>
          </div>
        {/if}

        <div class="safe">
          <Icon name="shield" size={16} />
          <span>
            {t("Files are not modified in place; all output is written to the destination folder.", settings.language)}
          </span>
        </div>
      </div>
    </div>

    <div class="act">
      <button class="go" onclick={startOrCancel} disabled={false}>
        <Icon name={running ? "stop" : "play-solid"} size={16} />
        {running ? t("Cancel", settings.language) : t("Start compression", settings.language)}
        <kbd>F5</kbd>
      </button>
      <button class="b2" disabled={scanning} onclick={scan}>
        <Icon name="scan" size={16} /> {t("Scan", settings.language)}
      </button>
      <button class="b2" onclick={() => goSettings("general")}>
        <Icon name="settings" size={16} /> {t("Settings", settings.language)}
      </button>
      <button class="b2" disabled={!lastStats} onclick={() => exportReport("csv")}>
        <Icon name="save" size={16} /> {t("Report", settings.language)}
      </button>
      <button class="b2 sq" onclick={openDestination} title={t("Open destination folder", settings.language)} aria-label={t("Open destination folder", settings.language)}>
        <Icon name="folder-target" size={18} />
      </button>
    </div>
  {/if}

  <div class="stbar">
    <Icon name="info" size={16} />
    <span>{$currentTip || t(DEFAULT_TIP, settings.language)}</span>
  </div>

  {#if toastMessage}
    <div class="toast {toastKind}" role="status">{toastMessage}</div>
  {/if}
</div>
