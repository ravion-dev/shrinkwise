<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import Icon from "../components/Icon.svelte";
  import { t, tn, localizeLog } from "../i18n";
  import Toggle from "../components/Toggle.svelte";
  import Slider from "../components/Slider.svelte";
  import type {
    CodecSupport,
    CpuBenchmark,
    EncoderSupport,
    Settings as AppSettings,
    SystemInfo,
    ToolInfo,
    ToolProgress,
    ToolsStatus,
  } from "../types";

  type Props = {
    settings: AppSettings;
    system: SystemInfo | null;
    tools: ToolsStatus | null;
    version: string;
    settingsPath: string;
    paths: Record<string, string>;
    section?: string;
    onToolsChanged: (tools: ToolsStatus) => void;
    onSystemChanged: (system: SystemInfo) => void;
    toast: (message: string, kind?: "ok" | "err") => void;
    onBack?: () => void;
  };

  let {
    settings = $bindable(),
    system,
    tools,
    version,
    settingsPath,
    paths,
    section = $bindable("general"),
    onToolsChanged,
    onSystemChanged,
    toast,
    onBack = () => {},
  }: Props = $props();

  const lang = $derived(settings.language);

  const nav = $derived([
    { id: "general", label: t("General", lang), icon: "settings" },
    { id: "hardware", label: t("Hardware", lang), icon: "cpu" },
    { id: "tools", label: t("Tools", lang), icon: "download" },
    { id: "compression", label: t("Compression", lang), icon: "sliders" },
    { id: "about", label: t("About", lang), icon: "info" },
  ]);

  let installing = $state<string | null>(null);
  let progress = $state<Record<string, ToolProgress>>({});
  let refreshingTools = $state(false);
  let refreshingTool = $state<string | null>(null);
  let justInstalled = $state<Record<string, boolean>>({});
  let refreshingSystem = $state(false);
  let encoderSupport = $state<EncoderSupport | null>(null);
  let detectingEncoders = $state(false);
  let benchmark = $state<CpuBenchmark | null>(null);
  let benchmarking = $state(false);

  onMount(() => {
    const unProgress = listen<ToolProgress>("tool-progress", (event) => {
      progress = { ...progress, [event.payload.id]: event.payload };
    });
    return () => {
      void unProgress.then((un) => un());
    };
  });

  function gb(mb: number): string {
    return `${(mb / 1024).toFixed(1)} GB`;
  }

  function toolPill(info: ToolInfo | undefined, installable = false) {
    if (!info) return { text: t("Not found", lang), cls: "warn" };
    if (info.available) {
      if (info.source === "system") return { text: t("Installed (System)", lang), cls: "ok" };
      if (info.source === "portable") return { text: t("Installed (Portable)", lang), cls: "ok" };
      return { text: t("Installed", lang), cls: "ok" };
    }
    if (installable) return { text: t("Not found", lang), cls: "warn" };
    return { text: info.required ? t("Required", lang) : t("Optional", lang), cls: "warn" };
  }

  const toolRows = $derived(
    [
      { key: "ffmpeg", installable: true, icon: "film" },
      { key: "ffprobe", installable: false, icon: "scan" },
      { key: "sevenZip", installable: true, icon: "archive" },
      { key: "realesrgan", installable: true, icon: "image" },
      { key: "waifu2x", installable: true, icon: "image" },
      { key: "ghostscript", installable: true, icon: "file-text" },
      { key: "qpdf", installable: true, icon: "shield" },
    ].map((row) => ({
      ...row,
      info: tools ? (tools as unknown as Record<string, ToolInfo>)[row.key] : undefined,
      id: tools ? (tools as unknown as Record<string, ToolInfo>)[row.key]?.id ?? row.key : row.key,
    })),
  );

  const encoderRows = $derived(
    encoderSupport
      ? [
          { label: "NVENC", support: encoderSupport.nvenc },
          { label: "AMF", support: encoderSupport.amf },
          { label: "QSV", support: encoderSupport.qsv },
        ]
      : [],
  );

  function codecText(support: CodecSupport): string {
    return [
      support.h264 ? "H.264" : "",
      support.hevc ? "HEVC" : "",
      support.av1 ? "AV1" : "",
    ]
      .filter(Boolean)
      .join(" · ");
  }

  function codecOk(support: CodecSupport): boolean {
    return support.h264 || support.hevc || support.av1;
  }

  function applyBenchmarkWorkers() {
    if (benchmark) settings.workers = benchmark.recommendedWorkers;
  }

  async function refreshTools(showToast = true) {
    refreshingTools = true;
    try {
      const status = await invoke<ToolsStatus>("refresh_tools", {
        settings: $state.snapshot(settings),
      });
      onToolsChanged(status);
      if (showToast) toast(t("Tool status refreshed", lang), "ok");
    } catch (err) {
      toast(String(err), "err");
    } finally {
      refreshingTools = false;
    }
  }

  async function refreshSystem() {
    refreshingSystem = true;
    try {
      const info = await invoke<SystemInfo>("refresh_system");
      onSystemChanged(info);
      toast(t("Hardware info updated", lang), "ok");
    } catch (err) {
      toast(String(err), "err");
    } finally {
      refreshingSystem = false;
    }
  }

  async function refreshSingleTool(key: string, id: string) {
    if (refreshingTools || refreshingTool) return;
    refreshingTool = id;
    try {
      const info = await invoke<ToolInfo>("refresh_tool", {
        settings: $state.snapshot(settings),
        id,
      });
      if (tools) {
        const next = { ...tools, [key]: info } as ToolsStatus;
        if (id === "ffmpeg") {
          const ffprobe = await invoke<ToolInfo>("refresh_tool", {
            settings: $state.snapshot(settings),
            id: "ffprobe",
          });
          next.ffprobe = ffprobe;
        }
        onToolsChanged(next);
      }
      toast(tn("{name} status refreshed", lang, { name: info.name }), "ok");
    } catch (err) {
      toast(String(err), "err");
    } finally {
      refreshingTool = null;
    }
  }

  async function installTool(id: string) {
    if (installing) return;
    installing = id;
    progress = {
      ...progress,
      [id]: {
        id,
        phase: "start",
        downloaded: 0,
        total: 0,
        percent: 0,
        message: t("Starting installation…", lang),
      },
    };
    try {
      const info = await invoke<ToolInfo>("install_tool", { id });
      justInstalled = { ...justInstalled, [id]: true };
      toast(
        tn("{name} ready", lang, {
          name: info.name,
        }) + (info.version ? ` · ${info.version}` : ""),
        "ok",
      );
      progress = {
        ...progress,
        [id]: { ...progress[id], phase: "done", percent: 100, message: t("Ready", lang) },
      };
      if (id === "ffmpeg") {
        await refreshTools(false);
      } else if (tools) {
        const key = id === "seven_zip" ? "sevenZip" : id;
        onToolsChanged({ ...tools, [key]: info } as ToolsStatus);
      }
    } catch (err) {
      toast(String(err), "err");
      const nextProgress = { ...progress };
      delete nextProgress[id];
      progress = nextProgress;
    } finally {
      installing = null;
    }
  }

  async function detectEncoders() {
    detectingEncoders = true;
    try {
      encoderSupport = await invoke<EncoderSupport>("detect_encoders", {
        settings: $state.snapshot(settings),
      });
      toast(
        encoderSupport.recommended === "cpu"
          ? t("No hardware encoder found; CPU will be used", lang)
          : tn("Recommended encoder: {x}", lang, {
              x: encoderSupport.recommended.toUpperCase(),
            }),
        encoderSupport.recommended === "cpu" ? "err" : "ok",
      );
    } catch (err) {
      toast(String(err), "err");
    } finally {
      detectingEncoders = false;
    }
  }

  async function runBenchmark() {
    benchmarking = true;
    try {
      benchmark = await invoke<CpuBenchmark>("benchmark_cpu");
      toast(
        tn("CPU score: {score} MB/s · {label}", lang, {
          score: benchmark.scoreMbps.toFixed(1),
          label: t(benchmark.label, lang),
        }),
        "ok",
      );
    } catch (err) {
      toast(String(err), "err");
    } finally {
      benchmarking = false;
    }
  }

  async function resetDefaults() {
    if (!window.confirm(t("All settings will revert to defaults. Continue?", lang))) return;
    try {
      const fresh = await invoke<AppSettings>("reset_settings");
      Object.assign(settings, fresh);
      toast(t("Settings reset to defaults", lang), "ok");
    } catch (err) {
      toast(String(err), "err");
    }
  }

  async function openFolder() {
    const target = paths.appDir ?? ".";
    try {
      await openPath(target);
    } catch (err) {
      toast(String(err), "err");
    }
  }

  async function revealSettings() {
    try {
      await revealItemInDir(settingsPath);
    } catch (err) {
      toast(String(err), "err");
    }
  }

  async function revealTool(info: ToolInfo | undefined) {
    if (!info?.path) return;
    try {
      await revealItemInDir(info.path);
    } catch (err) {
      toast(String(err), "err");
    }
  }
</script>

<div class="settings-wrap">
  <div class="settings-nav">
    <button class="nav-back" onclick={onBack}>
      <Icon name="chevron-right" size={15} class="flip" />
      {t("Back to compression", lang)}
    </button>
    {#each nav as item}
      <button class:active={section === item.id} onclick={() => (section = item.id)}>
        <Icon name={item.icon} size={16} />
        {item.label}
      </button>
    {/each}
    <div class="nav-note">
      <b>{t("Portable setup", lang)}</b><br />
      {t("Settings file", lang)} <span class="mono">settings.json</span>, {t("Tools folder", lang)} <span class="mono">tools/bin</span>.
      {t("Everything comes with you when you copy the folder.", lang)}
    </div>
  </div>

  <div class="settings-body">
    {#if section === "general"}
      <div>
        <h2 class="settings-title">{t("General", lang)}</h2>
        <div class="settings-sub">{t("Appearance, work mode and safety preferences.", lang)}</div>
      </div>

      <div class="info-card">
        <h3>{t("Appearance", lang)}</h3>
        <p>{t("Theme and interface scale apply instantly.", lang)}</p>
        <div class="row" style="margin-top:12px">
          <div class="mode-switch">
            <button class:active={settings.theme === "dark"} aria-pressed={settings.theme === "dark"} onclick={() => (settings.theme = "dark")}>
              <Icon name="moon" size={14} /> {t("Dark", lang)}
            </button>
            <button class:active={settings.theme === "light"} aria-pressed={settings.theme === "light"} onclick={() => (settings.theme = "light")}>
              <Icon name="sun" size={14} /> {t("Light", lang)}
            </button>
          </div>
        </div>
        <div style="margin-top:14px">
          <Slider
            label={t("Font scale", lang)}
            bind:value={settings.fontScale}
            min={0.8}
            max={1.6}
            step={0.05}
            suffix="×"
            hint={t("Larger interface", lang)}
          />
        </div>
        <div class="form-item" style="margin-top:14px; max-width:260px">
          <span class="lbl">{t("Interface language", lang)}</span>
          <select class="input" bind:value={settings.language}>
            <option value="tr">{t("Turkish", lang)}</option>
            <option value="en">{t("English", lang)}</option>
          </select>
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Work mode", lang)}</h3>
        <p>{t("Basic behaviors that determine how files are processed.", lang)}</p>
        <div style="margin-top:8px">
          <Toggle
            label={t("Simple mode", lang)}
            desc={t("Manage advanced options automatically in the background.", lang)}
            checked={settings.simpleMode}
            onchange={(v) => (settings.simpleMode = v)}
          />
          <Toggle
            label={t("Notify when done", lang)}
            desc={t("Show a desktop notification when the job finishes.", lang)}
            checked={settings.notifyDone}
            onchange={(v) => (settings.notifyDone = v)}
          />
          <Toggle
            label={t("Check tools on startup", lang)}
            desc={t("Scans FFmpeg version and encoder list on startup. When off, use Refresh on the Tools tab.", lang)}
            checked={settings.autoCheckTools}
            onchange={(v) => (settings.autoCheckTools = v)}
          />
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Security and verification", lang)}</h3>
        <p>{t("Options that protect your original files.", lang)}</p>
        <div style="margin-top:8px">
          <Toggle
            label={t("Keep larger output", lang)}
            desc={t("Copy the original if compression does not shrink the file.", lang)}
            checked={settings.keepLarger}
            onchange={(v) => (settings.keepLarger = v)}
          />
          <Toggle
            label={t("Keep metadata", lang)}
            desc={t("Preserve EXIF, title and tag info when possible.", lang)}
            checked={settings.keepMetadata}
            onchange={(v) => (settings.keepMetadata = v)}
          />
          <Toggle
            label={t("Keep modification time", lang)}
            desc={t("Match the output file's mtime to the source.", lang)}
            checked={settings.keepMtime}
            onchange={(v) => (settings.keepMtime = v)}
          />
          <Toggle
            label={t("Verify output", lang)}
            desc={t("Check that the encoded file can be opened.", lang)}
            checked={settings.verify}
            onchange={(v) => (settings.verify = v)}
          />
          <Toggle
            label={t("Dry run", lang)}
            desc={t("Show only the plan and estimate without writing any files.", lang)}
            checked={settings.dryRun}
            onchange={(v) => (settings.dryRun = v)}
          />
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Resource usage and filters", lang)}</h3>
        <div class="form-grid" style="margin-top:10px">
          <div class="form-item">
            <span class="lbl">{t("Concurrent workers", lang)}</span>
            <input class="input" type="number" min="1" max="32" bind:value={settings.workers} />
          </div>
          <div class="form-item">
            <span class="lbl">{t("Smallest file (KB)", lang)}</span>
            <input class="input" type="number" min="0" bind:value={settings.minSizeKb} />
          </div>
          <div class="form-item">
            <span class="lbl">{t("Largest file (MB, 0 = unlimited)", lang)}</span>
            <input class="input" type="number" min="0" bind:value={settings.limitMb} />
          </div>
          <div class="form-item wide">
            <span class="lbl">{t("Exclude patterns (comma separated)", lang)}</span>
            <input
              class="input"
              placeholder="*.tmp, node_modules, .git"
              bind:value={settings.excludeGlobs}
            />
          </div>
        </div>
      </div>
    {:else if section === "hardware"}
      <div class="row" style="align-items:flex-start">
        <div>
          <h2 class="settings-title">{t("Hardware", lang)}</h2>
          <div class="settings-sub">{t("Detects your system and recommends the best encoder.", lang)}</div>
        </div>
        <div class="grow"></div>
        <button class="btn" disabled={refreshingSystem} onclick={refreshSystem}>
          <Icon name="refresh" size={15} /> {t("Refresh", lang)}
        </button>
      </div>

      {#if system}
        <div class="hw-grid">
          <div class="hw-card">
            <div class="hw-head"><Icon name="cpu" size={14} /> {t("CPU", lang)}</div>
            <div class="hw-main">{system.cpu.brand || t("Unknown CPU", lang)}</div>
            <div class="hw-sub">
              {system.cpu.physicalCores} {t("physical", lang)} · {system.cpu.logicalCores} {t("logical cores", lang)}
              {#if system.cpu.frequencyMhz}· {system.cpu.frequencyMhz} MHz{/if}
            </div>
            <div class="kv"><span class="k">{t("Recommended workers", lang)}</span><span class="v">{system.recommendedWorkers}</span></div>
            <div class="kv"><span class="k">{t("Architecture", lang)}</span><span class="v">{system.arch}</span></div>
          </div>

          <div class="hw-card">
            <div class="hw-head"><Icon name="hard-drive" size={14} /> {t("Memory", lang)}</div>
            <div class="hw-main">{gb(system.memoryTotalMb)}</div>
            <div class="hw-sub">{t("Available", lang)}: {gb(system.memoryAvailableMb)}</div>
            <div class="kv"><span class="k">{t("Operating system", lang)}</span><span class="v">{system.os}</span></div>
            <div class="kv"><span class="k">{t("Recommended encoder", lang)}</span><span class="v">{system.recommendedEncoder.toUpperCase()}</span></div>
          </div>
        </div>

        <div class="info-card">
          <h3>{t("Graphics cards", lang)}</h3>
          <p>{t("Detected via DXGI; software adapters are filtered out.", lang)}</p>
          <div style="margin-top:12px; display:flex; flex-direction:column; gap:10px">
            {#each system.gpus.filter((g) => !g.software) as gpu}
              <div class="hw-card" style="padding:12px">
                <div class="row">
                  <span class="pill accent"><Icon name="gpu" size={12} /> {gpu.vendorLabel}</span>
                  {#if gpu.memoryMb > 0}<span class="pill">{gb(gpu.memoryMb)}</span>{/if}
                </div>
                <div class="hw-main" style="font-size:12.5px">{gpu.name}</div>
              </div>
            {:else}
              <div class="pill warn">{t("No hardware GPU found; CPU will be used.", lang)}</div>
            {/each}
          </div>
          {#if system.gpus.some((g) => g.software)}
            <p style="margin-top:10px">
              {system.gpus.filter((g) => g.software).length} {t("software adapter(s) ignored.", lang)}
            </p>
          {/if}
        </div>

        <div class="info-card">
          <h3>{t("Hardware acceleration", lang)}</h3>
          <div style="margin-top:8px">
            <Toggle
              label={t("Use GPU encoding", lang)}
              desc={t("Speeds up video jobs if NVENC / AMF / QSV is available.", lang)}
              checked={settings.gpu}
              onchange={(v) => (settings.gpu = v)}
            />
            <Toggle
              label={t("Allow AV1 encoder", lang)}
              desc={t("Allow GPU AV1 encoders (NVENC/AMF/QSV).", lang)}
              checked={settings.gpuAv1}
              onchange={(v) => (settings.gpuAv1 = v)}
            />
            <div class="form-grid" style="margin-top:10px">
              <div class="form-item">
                <span class="lbl">{t("Encoder preference", lang)}</span>
                <select class="input" bind:value={settings.encoderPrefer}>
                  <option value="quality">{t("Quality", lang)}</option>
                  <option value="speed">{t("Speed", lang)}</option>
                  <option value="balanced">{t("Balanced", lang)}</option>
                </select>
              </div>
              <div class="form-item">
                <span class="lbl">{t("Test status", lang)}</span>
                <button class="btn" disabled={detectingEncoders} onclick={detectEncoders}>
                  {#if detectingEncoders}<span class="spinner" style="width:14px;height:14px;border-width:2px"></span>{/if}
                  {t("Test encoders", lang)}
                </button>
              </div>
            </div>
          </div>
        </div>

        {#if encoderSupport}
          <div class="info-card">
            <div class="row">
              <h3 style="margin:0">{t("Encoder test", lang)}</h3>
              <div class="grow"></div>
              <span class="pill {encoderSupport.recommended === 'cpu' ? 'warn' : 'ok'}">
                {tn("Recommended: {x}", lang, { x: encoderSupport.recommended.toUpperCase() })}
              </span>
            </div>
            <div class="row" style="margin-top:12px">
              {#each encoderRows as row}
                <span class="pill {codecOk(row.support) ? 'ok' : ''}">
                  {row.label}:
                  {codecText(row.support) || t("none", lang)}
                </span>
              {/each}
            </div>
            {#if encoderSupport.note}<p style="margin-top:10px">{t(encoderSupport.note, lang)}</p>{/if}
          </div>
        {/if}

        <div class="info-card">
          <h3>{t("CPU benchmark", lang)}</h3>
          <p>{t("Measure your CPU's real speed with a short compression test.", lang)}</p>
          <div class="row" style="margin-top:12px">
            <button class="btn" disabled={benchmarking} onclick={runBenchmark}>
              {#if benchmarking}<span class="spinner" style="width:14px;height:14px;border-width:2px"></span>{/if}
              {t("Start benchmark", lang)}
            </button>
            {#if benchmark}
              <span class="pill accent">{benchmark.scoreMbps.toFixed(1)} MB/s · {t(benchmark.label, lang)}</span>
              <button class="btn tiny" onclick={applyBenchmarkWorkers}>
                {tn("Apply {n} worker suggestion", lang, { n: benchmark.recommendedWorkers })}
              </button>
            {/if}
          </div>
        </div>
      {:else}
        <div class="info-card"><p>{t("Hardware info not loaded yet.", lang)}</p></div>
      {/if}
    {:else if section === "tools"}
      <div class="row" style="align-items:flex-start">
        <div>
          <h2 class="settings-title">{t("Tools", lang)}</h2>
          <div class="settings-sub">{t("Install FFmpeg, 7-Zip and AI upscaling engines with one click.", lang)}</div>
        </div>
        <div class="grow"></div>
        <button class="btn" disabled={refreshingTools} onclick={() => refreshTools()}>
          <Icon name="refresh" size={15} /> {t("Refresh all", lang)}
        </button>
      </div>

      <div class="info-card">
        <div class="row">
          <span class="pill {tools?.ffmpeg.available ? 'ok' : 'warn'}">
            {tools?.ffmpeg.available ? t("Video/audio engine ready", lang) : t("FFmpeg must be installed", lang)}
          </span>
          {#if tools?.portable}<span class="pill accent">{t("Portable", lang)}</span>{/if}
          {#if tools?.encoders.length}
            <span class="pill">{tn("{n} encoders detected", lang, { n: [...new Set(tools.encoders)].length })}</span>
          {/if}
        </div>
        <p style="margin-top:10px">
          {t("FFmpeg is the heart of video and audio compression. Without it these files are only copied.", lang)}
          {t("The sha256 checksum is verified during download and files are extracted to the tools/bin folder next to the app.", lang)}
        </p>
        <div class="kv"><span class="k">{t("Tools folder", lang)}</span><span class="v">{tools?.toolsDir ?? paths.tools ?? "-"}</span></div>
      </div>

      {#each toolRows as row}
        <div class="tool-card">
          <div class="tool-head">
            <Icon name={row.icon} size={20} />
            <div style="min-width:0">
              <div class="tool-name">{row.info?.name ?? row.key}</div>
              <div class="tool-meta">
                {#if row.info?.path}
                  {row.info.path}
                {:else}
                  {t("Not found on system", lang)}
                {/if}
              </div>
              {#if row.info?.version}<div class="tool-meta" style="color:var(--good)">{row.info.version}</div>{/if}
              {#if row.info?.note}<div class="tool-meta">{t(row.info.note, lang)}</div>{/if}
            </div>
            <span class="pill {toolPill(row.info, row.installable).cls}">{toolPill(row.info, row.installable).text}</span>
          </div>

          <div class="tool-actions">
            {#if row.installable}
              {#if row.info?.available && justInstalled[row.id]}
                <button class="btn" disabled>
                  <Icon name="check" size={15} />
                  {t("Up to date", lang)}
                </button>
              {:else}
                <button
                  class="btn {row.info?.available ? '' : 'primary'}"
                  disabled={installing !== null}
                  onclick={() => installTool(row.id)}
                >
                  {#if installing === row.id}
                    <span class="spinner" style="width:14px;height:14px;border-width:2px"></span>
                    {t("Installing…", lang)}
                  {:else if row.info?.available}
                    <Icon name="refresh" size={14} />
                    {t("Reinstall", lang)}
                  {:else}
                    <Icon name="download" size={15} />
                    {t("One-click install", lang)}
                  {/if}
                </button>
              {/if}
            {:else if row.key === "ffprobe"}
              <span class="muted small">{t("Installed together with FFmpeg.", lang)}</span>
            {:else}
              <span class="muted small">
                {t("Optional external tool. Found automatically if installed; not downloaded separately.", lang)}
              </span>
            {/if}
            <button
              class="btn tiny"
              disabled={refreshingTools || refreshingTool !== null}
              onclick={() => refreshSingleTool(row.key, row.id)}
            >
              {#if refreshingTool === row.id}
                <span class="spinner" style="width:12px;height:12px;border-width:2px"></span>
              {:else}
                <Icon name="refresh" size={13} />
              {/if}
              {t("Refresh", lang)}
            </button>
            {#if row.info?.path}
              <button class="btn tiny" onclick={() => revealTool(row.info)}>
                <Icon name="folder-open" size={13} /> {t("Show in folder", lang)}
              </button>
            {/if}
          </div>

          {#if progress[row.id] && (installing === row.id || progress[row.id].phase !== "done")}
            <div class="install-progress">
              <div class="ip-top">
                <span>{localizeLog(progress[row.id].message, lang)}</span>
                <span>{progress[row.id].percent.toFixed(0)}%</span>
              </div>
              <div class="progress-track">
                <div
                  class="progress-fill"
                  style={`width:${Math.max(3, progress[row.id].percent)}%`}
                ></div>
              </div>
            </div>
          {/if}
        </div>
      {/each}

      <div class="info-card">
        <h3>{t("License note", lang)}</h3>
        <p>
          {t("The FFmpeg \"essentials\" build provided by gyan.dev contains libx264/libx265.", lang)}
          {t("If you plan commercial distribution, use your own FFmpeg build or review the license terms.", lang)}
        </p>
      </div>
    {:else if section === "compression"}
      <div>
        <h2 class="settings-title">{t("Compression", lang)}</h2>
        <div class="settings-sub">
          {t("Advanced per-type options. The lossy slider on the main screen scales these automatically.", lang)}
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Images", lang)}</h3>
        <div class="form-grid" style="margin-top:10px">
          <div class="form-item">
            <span class="lbl">{t("Image format", lang)}</span>
            <select class="input" bind:value={settings.imageFormat}>
              <option value="auto">{t("Automatic (by source)", lang)}</option>
              <option value="keep">{t("Keep original", lang)}</option>
              <option value="avif">AVIF</option>
              <option value="webp">WebP</option>
              <option value="jpg">JPEG</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Max resolution (MP, 0 = unlimited)", lang)}</span>
            <input class="input" type="number" min="0" step="0.5" bind:value={settings.imageMaxMp} />
          </div>
          <div class="form-item">
            <span class="lbl">{t("Quality (0 = automatic)", lang)}</span>
            <input class="input" type="number" min="0" max="100" bind:value={settings.imageQuality} />
          </div>
          <div class="form-item">
            <span class="lbl">{t("Longest side scale (%)", lang)}</span>
            <input class="input" type="number" min="10" max="100" bind:value={settings.imageMaxSidePct} />
          </div>
        </div>
        <div style="margin-top:8px">
          <Toggle
            label={t("Convert PNG to JPEG", lang)}
            desc={t("Produces smaller JPEG when there is no transparency.", lang)}
            checked={settings.convertPngToJpeg}
            onchange={(v) => (settings.convertPngToJpeg = v)}
          />
          <Toggle
            label={t("Convert BMP to JPEG", lang)}
            desc={t("BMP is uncompressed, so this yields large savings.", lang)}
            checked={settings.convertBmpToJpeg}
            onchange={(v) => (settings.convertBmpToJpeg = v)}
          />
        </div>
      </div>

      <div class="info-card">
        <h3>{t("GIF and animation", lang)}</h3>
        <div class="form-grid" style="margin-top:10px">
          <div class="form-item">
            <span class="lbl">{t("GIF processing", lang)}</span>
            <select class="input" bind:value={settings.gifMode}>
              <option value="auto">{t("Automatic", lang)}</option>
              <option value="gif">{t("Optimize as GIF", lang)}</option>
              <option value="webp">{t("Animated WebP", lang)}</option>
              <option value="mp4">{t("Convert to MP4", lang)}</option>
              <option value="keep">{t("Keep", lang)}</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Max FPS", lang)}</span>
            <select class="input" bind:value={settings.gifMaxFps}>
              <option value="auto">{t("Automatic", lang)}</option>
              <option value="60">60</option>
              <option value="30">30</option>
              <option value="24">24</option>
              <option value="15">15</option>
              <option value="10">10</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Max width (px, 0 = automatic)", lang)}</span>
            <input class="input" type="number" min="0" bind:value={settings.gifMaxWidth} />
          </div>
        </div>
        <Toggle
          label={t("Infinite loop", lang)}
          desc={t("If off, the animation plays only once.", lang)}
          checked={settings.gifLoop}
          onchange={(v) => (settings.gifLoop = v)}
        />
      </div>

      <div class="info-card">
        <h3>{t("Video", lang)}</h3>
        <div class="form-grid" style="margin-top:10px">
          <div class="form-item">
            <span class="lbl">{t("Codec", lang)}</span>
            <select class="input" bind:value={settings.videoCodec}>
              <option value="auto">{t("Automatic", lang)}</option>
              <option value="h264">H.264</option>
              <option value="h265">H.265 / HEVC</option>
              <option value="av1">AV1</option>
              <option value="vp9">VP9</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Resolution", lang)}</span>
            <select class="input" bind:value={settings.videoResolution}>
              <option value="keep">{t("Keep", lang)}</option>
              <option value="2160p">2160p</option>
              <option value="1440p">1440p</option>
              <option value="1080p">1080p</option>
              <option value="720p">720p</option>
              <option value="480p">480p</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Max FPS", lang)}</span>
            <select class="input" bind:value={settings.videoMaxFps}>
              <option value="keep">{t("Keep", lang)}</option>
              <option value="60">60</option>
              <option value="30">30</option>
              <option value="24">24</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Scale (%)", lang)}</span>
            <input class="input" type="number" min="25" max="100" bind:value={settings.videoScalePct} />
          </div>
          <div class="form-item">
            <span class="lbl">{t("Audio codec", lang)}</span>
            <select class="input" bind:value={settings.videoAudioCodec}>
              <option value="auto">{t("Automatic", lang)}</option>
              <option value="aac">AAC</option>
              <option value="opus">Opus</option>
              <option value="mp3">MP3</option>
              <option value="copy">{t("Copy", lang)}</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("Encoder tune", lang)}</span>
            <select class="input" bind:value={settings.videoTune}>
              <option value="auto">{t("Automatic", lang)}</option>
              <option value="film">{t("Film", lang)}</option>
              <option value="animation">{t("Animation", lang)}</option>
              <option value="grain">{t("Preserve grain", lang)}</option>
            </select>
          </div>
        </div>
        <div style="margin-top:8px">
          <Toggle
            label={t("10-bit encoding", lang)}
            desc={t("Reduces banding; some older players do not support it.", lang)}
            checked={settings.video10bit}
            onchange={(v) => (settings.video10bit = v)}
          />
          <Toggle
            label={t("Denoise", lang)}
            desc={t("Better compression for grainy videos.", lang)}
            checked={settings.videoDenoise}
            onchange={(v) => (settings.videoDenoise = v)}
          />
          <Toggle
            label={t("Fast start for web", lang)}
            desc={t("Moves the moov atom to the start of the file.", lang)}
            checked={settings.videoFaststart}
            onchange={(v) => (settings.videoFaststart = v)}
          />
          <Toggle
            label={t("Keep subtitles", lang)}
            desc={t("Add compatible subtitles to the output.", lang)}
            checked={settings.videoKeepSubs}
            onchange={(v) => (settings.videoKeepSubs = v)}
          />
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Audio", lang)}</h3>
        <div style="margin-top:8px">
          <Toggle
            label={t("Normalize audio", lang)}
            desc={t("Bring different files to a similar loudness level.", lang)}
            checked={settings.audioNormalize}
            onchange={(v) => (settings.audioNormalize = v)}
          />
          <Toggle
            label={t("Convert to mono", lang)}
            desc={t("Halves size if a single channel is enough.", lang)}
            checked={settings.audioMono}
            onchange={(v) => (settings.audioMono = v)}
          />
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Documents and archives", lang)}</h3>
        <p>{t("PDF recompression needs Ghostscript or qpdf. Office files and ZIP-based archives are rebuilt with maximum deflate.", lang)}</p>
        <div class="form-grid" style="margin-top:12px">
          <div class="form-item">
            <span class="lbl">{t("PDF mode", lang)}</span>
            <select class="input" bind:value={settings.pdfMode}>
              <option value="max">{t("Maximum compression", lang)}</option>
              <option value="balanced">{t("Balanced", lang)}</option>
              <option value="quality">{t("High quality", lang)}</option>
              <option value="text">{t("Text / vectors", lang)}</option>
            </select>
          </div>
          <div class="form-item">
            <span class="lbl">{t("PDF image DPI (0 = auto)", lang)}</span>
            <input class="input" type="number" min="0" max="600" bind:value={settings.pdfRasterDpi} />
          </div>
        </div>
        <div style="margin-top:8px">
          <Toggle
            label={t("Deep archive processing", lang)}
            desc={t("Recompress ZIP entries and nested archives. Uses more CPU.", lang)}
            checked={settings.zipDeep}
            onchange={(value) => (settings.zipDeep = value)}
          />
          {#if settings.zipDeep}
            <div class="form-item" style="max-width:220px">
              <span class="lbl">{t("Nested archive depth", lang)}</span>
              <input class="input" type="number" min="1" max="4" bind:value={settings.zipDeepDepth} />
            </div>
          {/if}
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Not yet supported types", lang)}</h3>
        <p>
          {t("This version compresses image, video, audio, GIF, text, SVG, office documents and ZIP-based archives. The following types are currently copied to the destination unchanged:", lang)}
        </p>
        <div class="row" style="margin-top:10px">
          <span class="pill warn">{t("Fonts", lang)}</span>
          <span class="pill warn">{t("Non-ZIP archives (RAR, 7z)", lang)}</span>
        </div>
        <p style="margin-top:10px">
          {t("Target-size and SSIM-based smart mode are not implemented yet; the lossy slider controls quality directly.", lang)}
        </p>
      </div>
    {:else}
      <div>
        <h2 class="settings-title">{t("About", lang)}</h2>
        <div class="settings-sub">ShrinkWise {version} · Rust + Tauri + Svelte</div>
      </div>

      <div class="info-card">
        <h3>ShrinkWise</h3>
        <p>
          {t("Shrink your files with a single lossy setting. Processes images, video, audio, GIF, PDF, office documents, archives, text, SVG and fonts in one pass. Picks the encoder suited to your hardware and installs required tools with one click.", lang)}
        </p>
        <div class="kv"><span class="k">{t("Version", lang)}</span><span class="v">{version}</span></div>
        <div class="kv"><span class="k">{t("Run mode", lang)}</span><span class="v">{paths.mode ?? "portable"}</span></div>
        <div class="kv"><span class="k">{t("Application folder", lang)}</span><span class="v">{paths.appDir ?? "-"}</span></div>
        <div class="kv"><span class="k">{t("Settings file", lang)}</span><span class="v">{settingsPath}</span></div>
        <div class="kv"><span class="k">{t("Tools folder", lang)}</span><span class="v">{paths.tools ?? "-"}</span></div>
        <div class="kv"><span class="k">{t("Temporary files", lang)}</span><span class="v">{paths.temp ?? "-"}</span></div>
        <div class="kv"><span class="k">{t("Logs", lang)}</span><span class="v">{paths.logs ?? "-"}</span></div>
      </div>

      <div class="info-card">
        <h3>{t("Portability", lang)}</h3>
        <p>
          {t("The app installs nothing and uses no registry. Settings, tools and logs are kept in the application folder. Copy the folder to a USB drive and run with the same settings on another computer.", lang)}
        </p>
        <div class="row" style="margin-top:12px">
          <button class="btn" onclick={openFolder}><Icon name="folder-open" size={15} /> {t("Open application folder", lang)}</button>
          <button class="btn" onclick={revealSettings}><Icon name="file-text" size={15} /> {t("Show settings", lang)}</button>
        </div>
      </div>

      <div class="info-card">
        <h3>{t("Thanks", lang)}</h3>
        <p>
          {t("Thanks to the FFmpeg, 7-Zip, Tauri, Svelte and Rust communities. This app uses open-source components; license details are in the third-party notices in the distribution folder.", lang)}
        </p>
        <div class="row" style="margin-top:12px">
          <button class="btn danger" onclick={resetDefaults}>
            <Icon name="refresh" size={15} /> {t("Reset to defaults", lang)}
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
