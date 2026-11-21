import { writable } from "svelte/store";

export const currentTip = writable<string>("");

let bubble: HTMLDivElement | null = null;
let activeNode: HTMLElement | null = null;
let activeText = "";

function bubbleElement(): HTMLDivElement {
  if (!bubble) {
    bubble = document.createElement("div");
    bubble.className = "tip";
    bubble.setAttribute("role", "tooltip");
    document.body.appendChild(bubble);
  }
  return bubble;
}

function position(node: HTMLElement) {
  const el = bubbleElement();
  el.textContent = activeText;
  el.classList.add("show");
  const rect = node.getBoundingClientRect();
  const margin = 8;
  const width = el.offsetWidth;
  const height = el.offsetHeight;
  const centered = rect.left + rect.width / 2 - width / 2;
  const left = Math.min(window.innerWidth - width - margin, Math.max(margin, centered));
  let top = rect.bottom + margin;
  if (top + height > window.innerHeight - 42) top = rect.top - height - margin;
  el.style.left = `${Math.round(left)}px`;
  el.style.top = `${Math.round(Math.max(margin, top))}px`;
}

function hideBubble() {
  bubble?.classList.remove("show");
  activeNode = null;
}

/** Svelte action: describes an element in the status bar and a floating tip. */
export function tip(node: HTMLElement, text: string) {
  let value = text;
  let hovered = false;
  let focused = false;
  let visible = false;
  let destroyed = false;

  const show = () => {
    if (visible) return;
    visible = true;
    activeNode = node;
    activeText = value;
    if (destroyed) return;
    currentTip.set(value);
    position(node);
  };
  const hide = () => {
    if (!visible) return;
    visible = false;
    if (!destroyed) currentTip.set("");
    hideBubble();
  };
  const refresh = () => {
    if (hovered || focused) show();
    else hide();
  };
  const onEnter = () => {
    hovered = true;
    refresh();
  };
  const onLeave = () => {
    hovered = false;
    refresh();
  };
  const onFocus = () => {
    focused = true;
    refresh();
  };
  const onBlur = () => {
    focused = false;
    refresh();
  };
  const onScroll = () => {
    if (visible) hide();
  };

  node.addEventListener("mouseenter", onEnter);
  node.addEventListener("mouseleave", onLeave);
  node.addEventListener("focus", onFocus);
  node.addEventListener("blur", onBlur);
  window.addEventListener("scroll", onScroll, true);

  return {
    update(next: string) {
      value = next;
      if (visible && !destroyed) {
        activeText = next;
        currentTip.set(next);
        if (activeNode === node) position(node);
      }
    },
    destroy() {
      destroyed = true;
      node.removeEventListener("mouseenter", onEnter);
      node.removeEventListener("mouseleave", onLeave);
      node.removeEventListener("focus", onFocus);
      node.removeEventListener("blur", onBlur);
      window.removeEventListener("scroll", onScroll, true);
      if (activeNode === node) hideBubble();
    },
  };
}

export const DEFAULT_TIP =
  "Hover over a setting to see what it does — its description appears here.";
