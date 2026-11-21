<script lang="ts">
  type Props = {
    label: string;
    value: number;
    min?: number;
    max?: number;
    step?: number;
    suffix?: string;
    hint?: string;
    low?: string;
    high?: string;
    onchange?: (value: number) => void;
  };
  let {
    label,
    value = $bindable(),
    min = 0,
    max = 100,
    step = 1,
    suffix = "",
    hint = "",
    low = "",
    high = "",
    onchange,
  }: Props = $props();

  const TICKS = [0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95, 100];

  const fill = $derived(max === min ? 0 : Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100)));
  const shown = $derived(step < 1 ? Number(value).toFixed(2) : String(Math.round(Number(value))));

  function tickLabel(percent: number): string {
    const next = min + ((max - min) * percent) / 100;
    return step < 1 ? next.toFixed(1) : String(Math.round(next));
  }

  function update(event: Event) {
    const next = Number((event.currentTarget as HTMLInputElement).value);
    value = next;
    onchange?.(next);
  }
</script>

<div class="slider-block">
  <div class="rhead">
    <b>{label}</b>
    <span class="big">{shown}<small>{suffix}</small></span>
  </div>
  <div class="ruler">
    <div class="base"></div>
    <div class="fill" style={`width:${fill}%`}></div>
    {#each TICKS as mark}
      <div class="t {mark % 25 === 0 ? 'm' : 's'}" style={`left:${mark}%`}></div>
    {/each}
    <div class="nm" style="left:0;transform:none">{tickLabel(0)}</div>
    <div class="nm" style="left:25%">{tickLabel(25)}</div>
    <div class="nm" style="left:50%">{tickLabel(50)}</div>
    <div class="nm" style="left:75%">{tickLabel(75)}</div>
    <div class="nm" style="left:100%;transform:translateX(-100%)">{tickLabel(100)}</div>
    {#if low}<div class="cap2" style="left:0">{low}</div>{/if}
    {#if high}<div class="cap2" style="right:0">{high}</div>{/if}
    <div class="th" style={`left:${fill}%`}></div>
    <input
      class="rng"
      type="range"
      {min}
      {max}
      {step}
      bind:value
      oninput={update}
      aria-label={label}
    />
  </div>
  {#if hint}<div class="hint"><span>{hint}</span></div>{/if}
</div>
