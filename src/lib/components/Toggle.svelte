<script lang="ts">
  type Props = {
    label: string;
    desc?: string;
    checked: boolean;
    onchange: (value: boolean) => void;
    disabled?: boolean;
  };
  let { label, desc = "", checked, onchange, disabled = false }: Props = $props();
</script>

<div class="row toggle">
  <div class="toggle-text">
    <b>{label}</b>
    {#if desc}<span>{desc}</span>{/if}
  </div>
  <button
    type="button"
    class="sw"
    class:on={checked}
    aria-pressed={checked}
    aria-label={label}
    {disabled}
    onclick={() => onchange(!checked)}
  ></button>
</div>
