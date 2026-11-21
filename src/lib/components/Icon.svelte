<script lang="ts">
  type Props = {
    name: string;
    size?: number;
    stroke?: number;
    class?: string;
  };
  let { name, size = 18, stroke = 1.75, class: cls = "" }: Props = $props();

  // Inner markup copied from ShrinkWise_Tasarim/assets/icons/svg so every glyph
  // uses the same 24px grid, 1.75 stroke, square caps and miter joins.
  const ICONS: Record<string, string> = {
    "bolt": '<path d="M13 3L5 13.5h6L10 21l8-10.5h-6z"/>',
    "braces": '<path d="M9 4.5c-2.2 0-3 .9-3 3v2c0 1.3-.7 2.2-2 2.5 1.3.3 2 1.2 2 2.5v2c0 2.1.8 3 3 3M15 4.5c2.2 0 3 .9 3 3v2c0 1.3.7 2.2 2 2.5-1.3.3-2 1.2-2 2.5v2c0 2.1-.8 3-3 3"/>',
    "check": '<path d="M4.5 12.5l5 5L19.5 7"/>',
    "chevron-down": '<path d="M5 9l7 7 7-7"/>',
    "chevron-right": '<path d="M9 5l7 7-7 7"/>',
    "chip": '<path d="M6.5 6.5h11v11h-11z"/><path d="M10 10h4v4h-4zM9.5 3v3.5M14.5 3v3.5M9.5 17.5V21M14.5 17.5V21M3 9.5h3.5M3 14.5h3.5M17.5 9.5H21M17.5 14.5H21"/>',
    "close": '<path d="M6 6l12 12M18 6L6 18"/>',
    "compress": '<path d="M4 4l6 6M10 5.5V10H5.5M20 4l-6 6M14 5.5V10h4.5M4 20l6-6M10 18.5V14H5.5M20 20l-6-6M14 18.5V14h4.5"/>',
    "download": '<path d="M12 4v11M7.2 10.5L12 15.2l4.8-4.7M4.5 19.5h15"/>',
    "error": '<circle cx="12" cy="12" r="9"/><path d="M8.5 8.5l7 7M15.5 8.5l-7 7"/>',
    "eye": '<path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12z"/><circle cx="12" cy="12" r="2.8"/>',
    "filter": '<path d="M3.5 5h17l-6.5 8v6l-4-2v-4z"/>',
    "folder": '<path d="M3 5.5h6.5l2 2.5H21v11.5H3z"/>',
    "folder-open": '<path d="M3 19.5v-14h6.5l2 2.5H19v3.5"/><path d="M3 19.5l3-8h16l-3 8z"/>',
    "folder-source": '<path d="M3 5.5h6.5l2 2.5H21v11.5H3z"/><path d="M12 11v6.5M9.2 14.8l2.8 2.7 2.8-2.7"/>',
    "folder-target": '<path d="M3 5.5h6.5l2 2.5H21v11.5H3z"/><path d="M12 17.5V11M9.2 13.7L12 11l2.8 2.7"/>',
    "gem": '<path d="M6.5 4.5h11l3.5 5-9 10-9-10z"/><path d="M3 9.5h18M9.5 4.5L8 9.5l4 10M14.5 4.5L16 9.5l-4 10"/>',
    "globe": '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3.2 3 3.2 15 0 18M12 3c-3.2 3-3.2 15 0 18"/>',
    "help": '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.4-1 .9-1 1.7M12 16.8v.01"/>',
    "info": '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.01"/>',
    "log": '<path d="M4 6h16M4 10.5h16M4 15h10M4 19.5h13"/>',
    "minus": '<path d="M5 12h14"/>',
    "moon": '<path d="M20 14.5A8.5 8.5 0 0 1 9.5 4 8.5 8.5 0 1 0 20 14.5z"/>',
    "play": '<path d="M7 4.5l13 7.5-13 7.5z"/>',
    "play-solid": '<path d="M7 4.5l13 7.5-13 7.5z" fill="currentColor"/>',
    "plus": '<path d="M12 5v14M5 12h14"/>',
    "ruler": '<path d="M2.5 8h19v8h-19z"/><path d="M7 8v3M11 8v4.5M15 8v3M19 8v4.5"/>',
    "scale": '<path d="M12 4v16M5 7h14M8 20h8M7 7l-3 7h6zM17 7l-3 7h6z"/>',
    "settings": '<path d="M10 3.5h4l.7 2.5 1.9 1.1 2.5-.7 2 3.5-1.8 1.8v2.6l1.8 1.8-2 3.5-2.5-.7-1.9 1.1-.7 2.5h-4l-.7-2.5-1.9-1.1-2.5.7-2-3.5 1.8-1.8v-2.6L3.9 9.9l2-3.5 2.5.7L10.3 6z"/><circle cx="12" cy="12" r="3"/>',
    "shield": '<path d="M12 3l8 3v6c0 4.5-3.2 7.8-8 9-4.8-1.2-8-4.5-8-9V6z"/><path d="M8.5 12l2.5 2.5 4.5-5"/>',
    "slider-single": '<path d="M3 12h7M14 12h7"/><path d="M10 8h4v8h-4z"/>',
    "sliders": '<path d="M3 7h10M17 7h4M3 17h4M11 17h10"/><path d="M13 4.5h4v5h-4zM7 14.5h4v5H7z"/>',
    "stop": '<path d="M6 6h12v12H6z"/>',
    "sun": '<circle cx="12" cy="12" r="4"/><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M18.4 5.6l-1.8 1.8M7.4 16.6l-1.8 1.8"/>',
    "table": '<path d="M3.5 4.5h17v15h-17zM3.5 9.5h17M3.5 14.5h17M10 9.5v10"/>',
    "target": '<circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="4.5"/><path d="M12 12v.01"/>',
    "terminal": '<path d="M3 4.5h18v15H3z"/><path d="M7 9.5l3 2.5-3 2.5M12.5 15H17"/>',
    "trash": '<path d="M4.5 6.5h15M9 6.5V4h6v2.5M6.5 6.5l1 13.5h9l1-13.5M10.2 10.5v6M13.8 10.5v6"/>',
    "type-archive": '<path d="M4 4.5h16V9H4z"/><path d="M5.5 9v10.5h13V9M10 13h4"/>',
    "type-audio": '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 10.5v3"/>',
    "type-font": '<path d="M5 20L12 4l7 16M7.8 14.5h8.4"/>',
    "type-gif": '<path d="M7 7V4.5h13.5v11H18"/><path d="M3.5 7.5H18v12H3.5z"/><path d="M9.5 11.5l4 2.5-4 2.5z"/>',
    "type-image": '<path d="M3.5 4.5h17v15h-17z"/><path d="M3.5 16l5-5 4 4 3-3 5 5"/><circle cx="16" cy="8.8" r="1.3"/>',
    "type-office": '<path d="M4 4h16v16H4z"/><path d="M4 9.5h16M9.5 9.5V20"/>',
    "type-other": '<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5"/>',
    "type-pdf": '<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 14h6M9 17.5h4"/>',
    "type-svg": '<path d="M5 19C5 10 11 5 19 5"/><path d="M3.5 17.5h3v3h-3zM17.5 3.5h3v3h-3z"/>',
    "type-text": '<path d="M8 8l-4 4 4 4M16 8l4 4-4 4M13.5 6l-3 12"/>',
    "type-video": '<path d="M3 6.5h13v11H3z"/><path d="M16 10.5l5-3v9l-5-3z"/>',
    "warning": '<path d="M12 3.5L22 20.5H2z"/><path d="M12 10v5M12 17.6v.01"/>',
    // App-only glyphs drawn on the same grid.
    "swap": '<path d="M6.5 7.5h11l-3.5-3.5M17.5 16.5h-11l3.5 3.5"/>',
    "scan": '<path d="M4 8V5.5h3.5M16.5 5.5H20V8M20 16v2.5h-3.5M7.5 18.5H4V16M4 12h16"/>',
    "refresh": '<path d="M20 11a8 8 0 0 0-13.7-4.7L4 8.5"/><path d="M4 4v4.5h4.5M20 13a8 8 0 0 1-13.7 4.7L4 15.5"/><path d="M20 20v-4.5h-4.5"/>',
    "upload": '<path d="M12 15.5V4M7.5 8.5L12 4l4.5 4.5M4.5 19.5h15"/>',
    "external": '<path d="M14 4h6v6M20 4l-9 9"/><path d="M18 14v5.5H4.5V6h5.5"/>',
    "sparkles": '<path d="M12 3l1.6 4.4L18 9l-4.4 1.6L12 15l-1.6-4.4L6 9l4.4-1.6z"/><path d="M18 15l.9 2.1L21 18l-2.1.9L18 21l-.9-2.1L15 18l2.1-.9z"/>',
    "file-text": '<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 13h6M9 17h6"/>',
    "package": '<path d="M21 8l-9-5-9 5 9 5z"/><path d="M3 8v8l9 5 9-5V8M12 13v8"/>',
    "monitor": '<path d="M3 4h18v12H3z"/><path d="M8 20h8M12 16v4"/>',
    "hard-drive": '<path d="M3 12l2-7a2 2 0 0 1 2-1h10a2 2 0 0 1 2 1l2 7v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M3 12h18M7 16h.5M11 16h.5"/>',
    "gpu": '<path d="M2.5 6.5h19v11h-19z"/><circle cx="9" cy="12" r="3"/><path d="M15 9.5h4M15 12h4M15 14.5h2"/>',
  };

  const ALIASES: Record<string, string> = {
    x: "close",
    alert: "warning",
    cpu: "chip",
    zap: "bolt",
    music: "type-audio",
    film: "type-video",
    image: "type-image",
    archive: "type-archive",
    list: "table",
    save: "download",
  };

  const markup = $derived(ICONS[ALIASES[name] ?? name] ?? ICONS["type-other"]);
</script>

<svg
  class={cls}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="square"
  stroke-linejoin="miter"
  stroke-miterlimit="10"
  aria-hidden="true"
>
  {@html markup}
</svg>
