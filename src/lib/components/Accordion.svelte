<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  type Props = {
    title: string;
    subtitle?: string;
    icon?: string;
    open?: boolean;
    children: Snippet;
  };
  let { title, subtitle = "", icon = "sliders", open = $bindable(false), children }: Props = $props();
</script>

<div class="acc" class:open>
  <button type="button" class="h" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name={icon} size={17} class="l" />
    <span class="txt">
      <b>{title}</b>
      {#if subtitle}<span>{subtitle}</span>{/if}
    </span>
    <span class="r"><Icon name={open ? "minus" : "plus"} size={15} /></span>
  </button>
  {#if open}
    <div class="bd">
      {@render children()}
    </div>
  {/if}
</div>
