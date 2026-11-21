<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import Icon from "$lib/components/Icon.svelte";
  import { t, localizeLog } from "$lib/i18n";
  import { formatBytes, formatDuration } from "$lib/types";
  import type {
    MediaInfo,
    Settings as AppSettings,
    ToolsStatus,
    UpscaleProgress,
    UpscaleResult,
  } from "$lib/types";

  type Props = {
    lang?: string;
    settings: AppSettings;
    tools?: ToolsStatus | null;
    drop?: { path: string; seq: number } | null;
    onOpenTools?: () => void;
  };

  let {
    lang = "en",
    settings,
    tools = null,
    drop = null,
    onOpenTools = () => {},
  }: Props = $props();

  const IMAGE_FILTERS = [
    { name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "tif", "tiff", "gif", "avif"] },
  ];
  const VIDEO_FILTERS = [
    { name: "Video", extensions: ["mp4", "mkv", "mov", "avi", "webm", "m4v", "wmv", "flv", "ts", "m2ts"] },
  ];

  let sub = $state<"image" | "video">("image");
  let info = $state<MediaInfo | null>(null);
  let loadingInfo = $state(false);
  let recipe = $state("auto");
  let model = $state("photo");
  let scale = $state(2);
  let target = $state("1080p");
  let quality = $state("balanced");
  let busy = $state(false);
  let progress = $state(0);
  let stage = $state("");
  let error = $state("");
  let result = $state<UpscaleResult | null>(null);
  let split = $state(50);
  let lastDropSeq = 0;

  const hasEsrgan = $derived(!!tools?.realesrgan?.available);
  const hasWaifu2x = $derived(!!tools?.waifu2x?.available);
  const hasFfmpeg = $derived(!!tools?.ffmpeg?.available);
  const isImage = $derived(sub === "image");

  const targets = [
    { id: "720p", label: "720p", height: 720 },
    { id: "1080p", label: "1080p", height: 1080 },
    { id: "1440p", label: "1440p", height: 1440 },
    { id: "2160p", label: "4K", height: 2160 },
    { id: "2x", label: "2×", factor: 2 },
    { id: "4x", label: "4×", factor: 4 },
  ];

  const chosenTarget = $derived(targets.find((item) => item.id === target) ?? targets[1]);
  const estimated = $derived.by(() => {
    if (!info || !info.width || !info.height) return "";
    const factor = isImage
      ? recipe === "waifu2x"
        ? 2
        : scale
      : (chosenTarget.factor ?? chosenTarget.height / info.height);
    const height = Math.round(info.height * factor);
    const width = Math.round((info.width * height) / info.height / 2) * 2;
    return `${width} × ${height}`;
  });

  const runGlyph = $derived(isImage ? "sparkles" : "film");

  onMount(() => {
    const un = listen<UpscaleProgress>("upscale-progress", (event) => {
      progress = Math.round(event.payload.percent);
      stage = localizeLog(event.payload.message, settings.language);
    });
    return () => {
      void un.then((off) => off());
    };
  });

  $effect(() => {
    if (recipe === "waifu2x" && scale !== 2) scale = 2;
  });

  $effect(() => {
    if (!drop || drop.seq === lastDropSeq) return;
    lastDropSeq = drop.seq;
    void load(drop.path);
  });

  function pickFilters(kind: "image" | "video") {
    return kind === "image" ? IMAGE_FILTERS : VIDEO_FILTERS;
  }

  async function pick(kind: "image" | "video") {
    if (busy) return;
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: pickFilters(kind),
      });
      if (typeof selected === "string") await load(selected);
    } catch (err) {
      error = String(err);
    }
  }

  async function load(path: string) {
    error = "";
    result = null;
    progress = 0;
    stage = "";
    split = 50;
    loadingInfo = true;
    try {
      const media = await invoke<MediaInfo>("inspect_media", {
        path,
        settings: $state.snapshot(settings),
      });
      info = media;
      sub = media.kind === "video" ? "video" : "image";
      if (media.kind === "video") {
        target =
          media.height >= 2160
            ? "2x"
            : media.height >= 1440
              ? "2160p"
              : media.height >= 1080
                ? "1440p"
                : "1080p";
      }
    } catch (err) {
      info = null;
      error = String(err);
    } finally {
      loadingInfo = false;
    }
  }

  function switchTab(next: "image" | "video") {
    if (busy || next === sub) return;
    sub = next;
    info = null;
    result = null;
    error = "";
    progress = 0;
    stage = "";
    split = 50;
  }

  function startProgress(message: string) {
    busy = true;
    result = null;
    error = "";
    progress = 2;
    stage = t(message, settings.language);
  }

  async function runImage() {
    if (!info || busy) return;
    startProgress("Preparing…");
    try {
      result = await invoke<UpscaleResult>("upscale_image", {
        path: info.path,
        scale,
        recipe,
        model,
        settings: $state.snapshot(settings),
      });
      progress = 100;
      split = 50;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function runVideo() {
    if (!info || busy) return;
    startProgress("Starting encoder…");
    try {
      result = await invoke<UpscaleResult>("upscale_video", {
        path: info.path,
        target,
        quality,
        settings: $state.snapshot(settings),
      });
      progress = 100;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function cancelRun() {
    try {
      await invoke("cancel_upscale");
    } catch {
      /* the running command reports the cancellation */
    }
  }

  async function reveal(path: string) {
    try {
      await revealItemInDir(path);
    } catch (err) {
      error = String(err);
    }
  }

  function targetEstimate(entry: { height?: number; factor?: number }): string {
    if (!info) return "";
    const height = Math.round(
      entry.factor ? info.height * entry.factor : (entry.height ?? info.height),
    );
    const width = Math.round((info.width * height) / info.height / 2) * 2;
    return `${width} × ${height}`;
  }

  function targetNote(entry: { height?: number; factor?: number }): string {
    if (!info) return "";
    const height = entry.factor ? info.height * entry.factor : (entry.height ?? info.height);
    if (height > info.height) return t("upscale", settings.language);
    if (height < info.height) return t("downscale", settings.language);
    return t("same size", settings.language);
  }

  function engineState(available: boolean): string {
    return available ? t("Installed", settings.language) : t("Not installed", settings.language);
  }

  function autoLabel(): string {
    if (hasEsrgan) return t("Uses Real-ESRGAN", settings.language);
    if (hasWaifu2x && scale === 2) return t("Uses Waifu2x", settings.language);
    return t("Uses Lanczos", settings.language);
  }

  function targetSize(entry: { height?: number; factor?: number }): string {
    if (entry.factor) return `${t("source", settings.language)} × ${entry.factor}`;
    const height = entry.height ?? 1080;
    const width = Math.round((height * 16) / 9 / 2) * 2;
    return `${width} × ${height}`;
  }
</script>

<div class="up">
  <div class="up-head">
    <div>
      <h2>{t("Upscaling", lang)}</h2>
      <p>{t("Enlarge images and videos with AI or high-quality resampling.", lang)}</p>
    </div>
    <div class="subseg">
      <button class:active={sub === "image"} aria-pressed={sub === "image"} onclick={() => switchTab("image")}>
        <Icon name="image" size={15} /> {t("Image Upscaling", lang)}
      </button>
      <button class:active={sub === "video"} aria-pressed={sub === "video"} onclick={() => switchTab("video")}>
        <Icon name="film" size={15} /> {t("Video Upscaling", lang)}
      </button>
    </div>
  </div>

  <div class="strip">
    <span class="cap">{isImage ? t("AI engines", lang) : t("Video engine", lang)}</span>
    {#if isImage}
      <div class="eng" class:ok={hasEsrgan}>
        <i class="dot"></i>
        <b>Real-ESRGAN</b>
        <span>{engineState(hasEsrgan)}</span>
        {#if !hasEsrgan}
          <button class="mini" onclick={onOpenTools}>{t("Install", lang)}</button>
        {/if}
      </div>
      <div class="eng" class:ok={hasWaifu2x}>
        <i class="dot"></i>
        <b>Waifu2x</b>
        <span>{engineState(hasWaifu2x)}</span>
        {#if !hasWaifu2x}
          <button class="mini" onclick={onOpenTools}>{t("Install", lang)}</button>
        {/if}
      </div>
    {/if}
    <div class="eng" class:ok={hasFfmpeg}>
      <i class="dot"></i>
      <b>FFmpeg</b>
      <span>{engineState(hasFfmpeg)}</span>
      {#if !hasFfmpeg}
        <button class="mini" onclick={onOpenTools}>{t("Install", lang)}</button>
      {/if}
    </div>
    <div class="spacer"></div>
    <button class="btn tiny" onclick={onOpenTools}>
      <Icon name="settings" size={13} /> {t("Manage tools", lang)}
    </button>
  </div>

  {#if error}
    <div class="error-box">
      <Icon name="warning" size={16} />
      <span>{error}</span>
    </div>
  {/if}

  <div class="work">
    <div class="side">
      {#if !info}
        <button class="drop" onclick={() => pick(isImage ? "image" : "video")} disabled={loadingInfo}>
          <span class="drop-icon"><Icon name={isImage ? "image" : "film"} size={26} /></span>
          {#if loadingInfo}
            <span class="spinner"></span>
            <b>{t("Reading file…", lang)}</b>
          {:else if isImage}
            <b>{t("Drop an image here", lang)}</b>
            <span>{t("PNG, JPEG, WebP, BMP, TIFF, GIF or AVIF — click to browse", lang)}</span>
          {:else}
            <b>{t("Drop a video here", lang)}</b>
            <span>{t("MP4, MKV, MOV, AVI or WebM — click to browse", lang)}</span>
          {/if}
        </button>
      {:else}
        <div class="file-card">
          <div class="thumb" class:video={!isImage}>
            {#if info.preview}
              <img src={info.preview} alt="" />
              {#if !isImage}<span class="play"><Icon name="play-solid" size={16} /></span>{/if}
            {:else}
              <Icon name={isImage ? "image" : "film"} size={22} />
            {/if}
          </div>
          <div class="file-meta">
            <b title={info.name}>{info.name}</b>
            <span>
              {info.width} × {info.height}
              {#if info.duration > 0} · {formatDuration(info.duration * 1000, lang)}{/if}
            </span>
            <span>{formatBytes(info.bytes)}</span>
          </div>
          <button class="icon-btn" onclick={() => pick(isImage ? "image" : "video")} title={t("Change file", lang)} aria-label={t("Change file", lang)}>
            <Icon name="swap" size={15} />
          </button>
        </div>
      {/if}

      <div class="card">
          <div class="card-h">
            <b>{isImage ? t("Upscaling engine", lang) : t("Target resolution", lang)}</b>
            <span class="cap">{isImage ? `${recipe === "waifu2x" ? 2 : scale}×` : chosenTarget.label}</span>
          </div>
          <div class="card-b">
            {#if isImage}
              <div class="opts">
                <button class="opt" class:on={recipe === "auto"} aria-pressed={recipe === "auto"} onclick={() => (recipe = "auto")}>
                  <span class="ic"><Icon name="sparkles" size={14} /></span>
                  <span class="txt">
                    <b>{t("Auto", lang)}</b>
                    <span>{autoLabel()}</span>
                  </span>
                </button>
                <button class="opt" class:on={recipe === "realesrgan"} aria-pressed={recipe === "realesrgan"} disabled={!hasEsrgan} onclick={() => (recipe = "realesrgan")}>
                  <span class="ic"><Icon name="gem" size={14} /></span>
                  <span class="txt">
                    <b>Real-ESRGAN</b>
                    <span>{hasEsrgan ? t("AI · photos and anime", lang) : t("Not installed", lang)}</span>
                  </span>
                </button>
                <button class="opt" class:on={recipe === "waifu2x"} aria-pressed={recipe === "waifu2x"} disabled={!hasWaifu2x} onclick={() => (recipe = "waifu2x")}>
                  <span class="ic"><Icon name="sparkles" size={14} /></span>
                  <span class="txt">
                    <b>Waifu2x</b>
                    <span>{hasWaifu2x ? t("AI · anime, 2x only", lang) : t("Not installed", lang)}</span>
                  </span>
                </button>
                <button class="opt" class:on={recipe === "lanczos"} aria-pressed={recipe === "lanczos"} onclick={() => (recipe = "lanczos")}>
                  <span class="ic"><Icon name="scale" size={14} /></span>
                  <span class="txt">
                    <b>{t("Lanczos", lang)}</b>
                    <span>{t("Built-in · fast", lang)}</span>
                  </span>
                </button>
              </div>

              <div class="field-row">
                {#if recipe === "realesrgan" || (recipe === "auto" && hasEsrgan)}
                  <div class="field">
                    <span class="lbl">{t("Model", lang)}</span>
                    <div class="seg2">
                      <button class:active={model === "photo"} aria-pressed={model === "photo"} onclick={() => (model = "photo")}>
                        {t("Photo", lang)}
                      </button>
                      <button class:active={model === "anime"} aria-pressed={model === "anime"} onclick={() => (model = "anime")}>
                        {t("Anime", lang)}
                      </button>
                    </div>
                  </div>
                {/if}
                <div class="field">
                  <span class="lbl">{t("Scale", lang)}</span>
                  <div class="seg2">
                    <button class:active={scale === 2} aria-pressed={scale === 2} onclick={() => (scale = 2)}>2×</button>
                    <button class:active={scale === 4} aria-pressed={scale === 4} disabled={recipe === "waifu2x"} onclick={() => (scale = 4)}>4×</button>
                  </div>
                </div>
              </div>
            {:else}
              <div class="targets">
                {#each targets as item}
                  <button class="target" class:on={target === item.id} aria-pressed={target === item.id} onclick={() => (target = item.id)}>
                    <b>{item.label}</b>
                    <span>{targetEstimate(item) || targetSize(item)}</span>
                    {#if info}<em>{targetNote(item)}</em>{/if}
                  </button>
                {/each}
              </div>

              <div class="field">
                <span class="lbl">{t("Quality", lang)}</span>
                <div class="seg2">
                  <button class:active={quality === "balanced"} aria-pressed={quality === "balanced"} onclick={() => (quality = "balanced")}>
                    {t("Balanced", lang)}
                  </button>
                  <button class:active={quality === "quality"} aria-pressed={quality === "quality"} onclick={() => (quality = "quality")}>
                    {t("High", lang)}
                  </button>
                </div>
              </div>

              {#if !hasFfmpeg}
                <div class="notice">
                  <Icon name="warning" size={15} />
                  {t("FFmpeg is required for video upscaling. Install it from Tools.", lang)}
                </div>
              {/if}
            {/if}
          </div>
        </div>

        <div class="run-bar">
          <div class="run-meta">
            <span class="cap">{t("Estimated output", lang)}</span>
            <b>{estimated || "—"}</b>
          </div>
          <button class="go" class:idle={!info} disabled={busy || !info || (!isImage && !hasFfmpeg)} onclick={() => (isImage ? runImage() : runVideo())}>
            {#if busy}
              <span class="spinner on-accent" style="width:16px;height:16px;border-width:2px"></span>
              {t("Processing", lang)}
            {:else if !info}
              <Icon name={isImage ? "image" : "film"} size={18} />
              {t("Choose a file to start", lang)}
            {:else}
              <Icon name={runGlyph} size={18} />
              {isImage ? t("Upscale image", lang) : t("Upscale video", lang)}
            {/if}
          </button>
        </div>
    </div>

    <div class="stage">
      <div class="stage-top">
        <span class="cap">{result ? t("Result", lang) : t("Preview", lang)}</span>
        {#if result}
          <span class="stage-pill">{result.engine}</span>
        {:else if info}
          <span class="stage-dims">{info.width} × {info.height}</span>
        {/if}
      </div>

      {#if result}
        {#if isImage && result.afterPreview}
          <div class="compare">
            <img class="after" src={result.afterPreview} alt={t("After", lang)} />
            <div class="before-clip" style={`clip-path: inset(0 ${100 - split}% 0 0)`}>
              <img src={result.beforePreview} alt={t("Before", lang)} />
            </div>
            <div class="handle" style={`left:${split}%`}>
              <span class="grip"><Icon name="swap" size={13} /></span>
            </div>
            <span class="ptag before">{t("Before", lang)}</span>
            <span class="ptag after">{t("After", lang)}</span>
            <input class="split" type="range" min="0" max="100" bind:value={split} aria-label={t("Before and after comparison", lang)} />
          </div>
          <div class="compare-hint">{t("Drag the slider to compare before and after.", lang)}</div>
        {:else if !isImage && info?.preview}
          <div class="compare video">
            <img src={info.preview} alt="" />
            <span class="ptag after">{result.width} × {result.height}</span>
          </div>
        {/if}

        <div class="readout">
          <div><span>{t("Output", lang)}</span><b>{result.width} × {result.height}</b></div>
          <div><span>{t("Size", lang)}</span><b>{formatBytes(result.bytes)}</b></div>
          <div><span>{t("Elapsed", lang)}</span><b>{formatDuration(result.elapsedMs, lang)}</b></div>
        </div>
        <div class="stage-actions">
          <button class="dark-btn" onclick={() => result && reveal(result.output)}>
            <Icon name="folder-open" size={14} /> {t("Show in folder", lang)}
          </button>
          <button class="dark-btn" disabled={busy} onclick={() => (isImage ? runImage() : runVideo())}>
            <Icon name="refresh" size={14} /> {t("Run again", lang)}
          </button>
          <span class="path" title={result.output}>{result.output}</span>
        </div>
      {:else if info?.preview}
        <div class="viewer">
          <img src={info.preview} alt="" />
          {#if !isImage}
            <span class="play big"><Icon name="play-solid" size={24} /></span>
          {/if}
        </div>
        <div class="stage-foot">
          <span>{isImage ? t("Source size", lang) : t("Source", lang)}: {info.width} × {info.height}</span>
          <span>{isImage ? `${scale}×` : `→ ${estimated}`}</span>
        </div>
      {:else}
        <div class="empty">
          <span class="drop-icon"><Icon name={isImage ? "image" : "film"} size={30} /></span>
          <b>{isImage ? t("Choose an image to start.", lang) : t("Choose a video to start.", lang)}</b>
          <span class="intro">{isImage ? t("PNG, JPEG, WebP, BMP, TIFF, GIF or AVIF — click to browse", lang) : t("MP4, MKV, MOV, AVI or WebM — click to browse", lang)}</span>
          <div class="guide">
            <div>
              <em>01</em>
              <b>{t("Choose a file", lang)}</b>
              <span>{t("Drop a file here or browse from disk.", lang)}</span>
            </div>
            <div>
              <em>02</em>
              <b>{t("Pick settings", lang)}</b>
              <span>{t("Choose an engine and scale, or a target resolution.", lang)}</span>
            </div>
            <div>
              <em>03</em>
              <b>{t("Save the result", lang)}</b>
              <span>{t("The upscaled file is written to the Upscaled folder.", lang)}</span>
            </div>
          </div>
          <div class="privacy"><Icon name="shield" size={14} /> {t("Runs locally. Originals are never modified.", lang)}</div>
        </div>
      {/if}

      {#if busy}
        <div class="job">
          <div class="job-top">
            <span class="cap">{t("Processing", lang)}</span>
            <span>{stage || t("Working…", lang)}</span>
          </div>
          <div class="pct-line">
            <b>{progress}<small>%</small></b>
            {#if busy}
              <button class="dark-btn" onclick={cancelRun}>
                <Icon name="stop" size={13} /> {t("Cancel", lang)}
              </button>
            {/if}
          </div>
          <div class="track"><i style={`width:${Math.max(progress, 2)}%`}></i></div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .up { display: flex; flex-direction: column; gap: 14px; padding: 4px 0 24px; }
  .up-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
  .up-head h2 { font-size: 19px; font-weight: 600; letter-spacing: -0.01em; }
  .up-head p { margin-top: 3px; font-size: 12px; color: var(--ink2); }
  .subseg { display: inline-flex; border: 1px solid var(--line-strong); border-radius: 3px; overflow: hidden; }
  .subseg button { padding: 8px 14px; border: 0; background: transparent; color: var(--ink2); cursor: pointer; display: flex; align-items: center; gap: 8px; font-weight: 500; font-size: 13px; }
  .subseg button.active { background: var(--sel-bg); color: var(--sel-fg); }
  .strip { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; padding: 7px 10px; border: 1px solid var(--line); border-radius: 3px; background: var(--surface); }
  .strip .cap { margin-right: 4px; }
  .eng { display: flex; align-items: center; gap: 6px; padding: 3px 8px; border: 1px solid var(--line); border-radius: 2px; background: var(--bg); font-size: 11.5px; color: var(--ink2); }
  .eng b { color: var(--ink); font-weight: 500; }
  .eng .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--ink3); }
  .eng.ok .dot { background: var(--good); }
  .eng .mini { border: 0; padding: 0; background: transparent; color: var(--accent); font-size: 11px; font-weight: 600; cursor: pointer; text-decoration: underline; }
  .spacer { flex: 1; }
  .error-box { display: flex; gap: 8px; align-items: flex-start; padding: 10px 12px; border: 1px solid var(--bad); color: var(--bad); background: var(--surface); border-radius: 3px; font-size: 12px; }
  .work { display: grid; grid-template-columns: minmax(310px, 400px) minmax(0, 1fr); gap: 16px; align-items: stretch; }
  .side { display: flex; flex-direction: column; gap: 12px; min-width: 0; }

  .drop { position: relative; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; min-height: 206px; padding: 26px; border: 1px dashed var(--line-strong); border-radius: 3px; background: var(--surface); color: var(--ink2); cursor: pointer; text-align: center; }
  .drop:hover { border-color: var(--accent); color: var(--ink); }
  .drop b { font-size: 14px; color: var(--ink); font-weight: 600; }
  .drop span { font-size: 11.5px; line-height: 1.5; max-width: 300px; }
  .drop:disabled { cursor: progress; }
  .drop-icon { width: 52px; height: 52px; display: grid; place-items: center; border: 1px solid var(--line); background: var(--bg); color: var(--ink3); margin-bottom: 2px; }
  .drop:hover .drop-icon { color: var(--accent); border-color: var(--accent); }

  .file-card { display: flex; align-items: center; gap: 12px; padding: 10px; background: var(--surface); border: 1px solid var(--line); border-radius: 3px; }
  .thumb { width: 108px; height: 66px; flex: none; display: grid; place-items: center; background: var(--sunken); border: 1px solid var(--line); border-radius: 2px; overflow: hidden; position: relative; color: var(--ink3); }
  .thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .thumb .play { position: absolute; inset: 0; display: grid; place-items: center; background: rgba(0, 0, 0, 0.35); color: #fff; }
  .file-meta { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .file-meta b { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-meta span { font: 10.5px PlexMono; color: var(--ink2); }
  .file-card .icon-btn { width: 30px; height: 30px; }

  .card { background: var(--surface); border: 1px solid var(--line); border-radius: 3px; }
  .card-h { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 9px 12px; border-bottom: 1px solid var(--line); }
  .card-h b { font-size: 12.5px; font-weight: 600; }
  .card-b { padding: 12px; display: flex; flex-direction: column; gap: 12px; }
  .cap { font: 500 10px PlexMono; letter-spacing: 0.08em; text-transform: uppercase; color: var(--ink2); }

  .opts { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
  .opt { display: flex; align-items: flex-start; gap: 8px; padding: 8px 9px; border: 1px solid var(--line); border-radius: 3px; background: var(--bg); color: var(--ink2); cursor: pointer; text-align: left; min-width: 0; }
  .opt .ic { width: 24px; height: 24px; flex: none; display: grid; place-items: center; border: 1px solid var(--line); background: var(--surface); color: var(--ink2); }
  .opt .txt { min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .opt b { color: var(--ink); font-size: 12px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .opt .txt span { font-size: 10px; line-height: 1.3; color: var(--ink2); }
  .opt:hover:not(:disabled) { border-color: var(--line-strong); }
  .opt.on { border-color: var(--accent); box-shadow: inset 2px 0 0 var(--accent); }
  .opt.on .ic { color: var(--accent); border-color: var(--accent); }
  .opt:disabled { opacity: 0.5; cursor: not-allowed; }

  .field-row { display: flex; align-items: center; gap: 18px; flex-wrap: wrap; }
  .field { display: flex; align-items: center; gap: 9px; }
  .lbl { font: 500 10px PlexMono; text-transform: uppercase; letter-spacing: 0.07em; color: var(--ink2); }
  .seg2 { display: inline-flex; border: 1px solid var(--line-strong); border-radius: 3px; overflow: hidden; }
  .seg2 button { padding: 5px 12px; border: 0; background: transparent; color: var(--ink); cursor: pointer; font-size: 12px; font-weight: 500; }
  .seg2 button + button { border-left: 1px solid var(--line); }
  .seg2 button.active { background: var(--sel-bg); color: var(--sel-fg); }
  .seg2 button:disabled { opacity: 0.4; cursor: not-allowed; }

  .targets { display: grid; grid-template-columns: repeat(3, 1fr); gap: 7px; }
  .target { display: flex; flex-direction: column; align-items: flex-start; gap: 1px; padding: 8px 9px; border: 1px solid var(--line); border-radius: 3px; background: var(--bg); color: var(--ink2); cursor: pointer; text-align: left; }
  .target b { font: 500 12.5px PlexMono; color: var(--ink); }
  .target span { font: 10px PlexMono; color: var(--ink2); }
  .target em { font: 9.5px PlexMono; font-style: normal; color: var(--ink3); text-transform: uppercase; letter-spacing: 0.04em; }
  .target:hover { border-color: var(--line-strong); }
  .target.on { border-color: var(--accent); box-shadow: inset 2px 0 0 var(--accent); }
  .target.on b { color: var(--accent); }
  .notice { display: flex; align-items: center; gap: 8px; padding: 8px 10px; border: 1px solid var(--warn); border-radius: 3px; color: var(--warn); font-size: 11.5px; }

  .run-bar { display: flex; flex-direction: column; gap: 8px; }
  .run-meta { display: flex; align-items: baseline; justify-content: space-between; }
  .run-meta b { font: 500 12px PlexMono; color: var(--ink); }
  .go { display: flex; align-items: center; justify-content: center; gap: 9px; height: 46px; border: 0; border-radius: 3px; background: var(--accent); color: var(--on-accent); font-weight: 600; font-size: 13.5px; cursor: pointer; }
  .go:hover:not(:disabled) { background: var(--accent-hover); }
  .go:disabled { opacity: 0.5; cursor: not-allowed; }
  .go.idle { opacity: 1; background: var(--surface); border: 1px solid var(--line-strong); color: var(--ink2); }
  .spinner.on-accent { border-color: rgba(255, 255, 255, 0.35); border-top-color: currentColor; }

  .stage { background: var(--panel); color: var(--panel-ink); border: 1px solid var(--panel-line); border-radius: 4px; display: flex; flex-direction: column; min-width: 0; overflow: hidden; }
  .stage-top { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 10px 14px; border-bottom: 1px solid var(--panel-line); }
  .stage-top .cap { color: var(--panel-ink2); }
  .stage-pill { border: 1px solid var(--panel-line); padding: 2px 7px; font: 500 10px PlexMono; color: var(--panel-accent); }
  .stage-dims { font: 11px PlexMono; color: var(--panel-ink2); }
  .viewer { position: relative; flex: 1; display: grid; place-items: center; min-height: 330px; padding: 12px; background-color: #0b0a09; background-image: linear-gradient(45deg, rgba(255, 255, 255, 0.035) 25%, transparent 25%, transparent 75%, rgba(255, 255, 255, 0.035) 75%), linear-gradient(45deg, rgba(255, 255, 255, 0.035) 25%, transparent 25%, transparent 75%, rgba(255, 255, 255, 0.035) 75%); background-size: 18px 18px; background-position: 0 0, 9px 9px; }
  .viewer img { max-width: 100%; max-height: 440px; object-fit: contain; display: block; }
  .play.big { position: absolute; inset: 0; margin: auto; width: 54px; height: 54px; border-radius: 50%; display: grid; place-items: center; background: rgba(0, 0, 0, 0.45); border: 1px solid rgba(255, 255, 255, 0.3); color: #fff; }
  .stage-foot { display: flex; align-items: center; justify-content: space-between; gap: 10px; padding: 9px 14px; border-top: 1px solid var(--panel-line); font: 10.5px PlexMono; color: var(--panel-ink2); }
  .empty { flex: 1; min-height: 330px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 9px; padding: 28px; text-align: center; color: var(--panel-ink2); }
  .empty .drop-icon { border-color: var(--panel-line); background: transparent; color: var(--panel-ink2); }
  .empty b { color: var(--panel-ink); font-size: 13.5px; }
  .empty .intro { font-size: 11px; line-height: 1.5; max-width: 320px; }
  .empty .guide { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; width: 100%; max-width: 620px; margin-top: 20px; text-align: left; }
  .empty .guide div { border-top: 1px solid var(--panel-line); padding-top: 10px; display: flex; flex-direction: column; gap: 4px; }
  .empty .guide em { font: 500 10px PlexMono; font-style: normal; letter-spacing: 0.06em; color: var(--panel-accent); }
  .empty .guide b { font-size: 11.5px; color: var(--panel-ink); }
  .empty .guide span { font-size: 10.5px; line-height: 1.45; color: var(--panel-ink2); }
  .empty .privacy { display: flex; align-items: center; gap: 7px; margin-top: 16px; font-size: 10.5px; color: var(--panel-ink2); }

  .compare { position: relative; height: clamp(280px, 40vh, 480px); background-color: #0b0a09; overflow: hidden; }
  .compare img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; display: block; }
  .compare.video { height: auto; min-height: 300px; }
  .before-clip { position: absolute; inset: 0; }
  .handle { position: absolute; top: 0; bottom: 0; width: 2px; background: var(--panel-accent); transform: translateX(-1px); pointer-events: none; z-index: 2; }
  .grip { position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 30px; height: 30px; border-radius: 50%; background: var(--panel-accent); color: #1a0a04; display: grid; place-items: center; box-shadow: 0 1px 8px rgba(0, 0, 0, 0.55); }
  .ptag { position: absolute; top: 10px; padding: 3px 7px; font: 500 9.5px PlexMono; letter-spacing: 0.05em; text-transform: uppercase; background: rgba(0, 0, 0, 0.6); color: #fff; border: 1px solid rgba(255, 255, 255, 0.16); border-radius: 2px; pointer-events: none; z-index: 2; }
  .ptag.before { left: 10px; }
  .ptag.after { right: 10px; }
  .split { position: absolute; inset: 0; width: 100%; height: 100%; margin: 0; opacity: 0; cursor: ew-resize; z-index: 3; }
  .split:focus-visible { outline: 2px solid var(--panel-accent); outline-offset: -2px; }
  .compare-hint { font-size: 10.5px; color: var(--panel-ink2); padding: 8px 14px 0; }

  .readout { display: grid; grid-template-columns: repeat(3, 1fr); border-top: 1px solid var(--panel-line); margin-top: auto; }
  .readout div { padding: 11px 14px; }
  .readout div + div { border-left: 1px solid var(--panel-line); }
  .readout span { display: block; font: 500 9px PlexMono; letter-spacing: 0.08em; text-transform: uppercase; color: var(--panel-ink2); }
  .readout b { display: block; margin-top: 4px; font: 500 14px PlexMono; color: var(--panel-ink); }
  .stage-actions { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; padding: 10px 14px; border-top: 1px solid var(--panel-line); }
  .dark-btn { height: 30px; border: 1px solid var(--panel-line); background: transparent; color: var(--panel-ink); display: inline-flex; align-items: center; gap: 6px; padding: 0 10px; border-radius: 3px; cursor: pointer; font-size: 11.5px; flex: none; }
  .dark-btn:hover:not(:disabled) { border-color: var(--panel-ink2); }
  .dark-btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .path { font: 10px PlexMono; color: var(--panel-ink2); overflow-wrap: anywhere; min-width: 0; }

  .job { padding: 12px 14px 14px; border-top: 1px solid var(--panel-line); display: flex; flex-direction: column; gap: 7px; }
  .job-top { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  .job-top .cap { color: var(--panel-ink2); }
  .job-top span:last-child { font: 10.5px PlexMono; color: var(--panel-ink2); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 62%; }
  .pct-line { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; }
  .pct-line b { font: 500 38px/1 PlexMono; letter-spacing: -0.03em; color: var(--panel-ink); }
  .pct-line b small { font-size: 15px; color: var(--panel-ink2); letter-spacing: 0; margin-left: 1px; }
  .track { height: 8px; background: var(--panel-line); overflow: hidden; }
  .track i { display: block; height: 100%; background: var(--panel-accent); transition: width 0.2s linear; }

  @media (max-width: 860px) {
    .work { grid-template-columns: 1fr; }
  }

  @media (max-width: 900px) {
    .empty .guide { grid-template-columns: 1fr; max-width: 420px; }
  }

  @media (max-width: 700px) {
    .opts { grid-template-columns: 1fr; }
    .targets { grid-template-columns: repeat(2, 1fr); }
  }
</style>
