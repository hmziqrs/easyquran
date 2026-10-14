<script lang="ts">
  import * as Select from "#lib/components/ui/select/index.js";

  /**
   * shadcn time picker for the card schedule: hour + minute selects bound to
   * a strict `HH:MM` string (empty until both halves are picked). Minutes
   * step by 5; an off-grid stored minute stays selectable so editing never
   * rewrites it silently.
   */
  let {
    value = $bindable(""),
    label,
    hourLabel,
    minuteLabel,
  }: {
    value: string;
    label: string;
    hourLabel: string;
    minuteLabel: string;
  } = $props();

  const pad = (n: number): string => String(n).padStart(2, "0");
  const HOURS = Array.from({ length: 24 }, (_, i) => pad(i));
  const MINUTE_STEPS = Array.from({ length: 12 }, (_, i) => pad(i * 5));

  const hour = $derived(/^\d{2}:/u.test(value) ? value.slice(0, 2) : "");
  const minute = $derived(/:\d{2}$/u.test(value) ? value.slice(3, 5) : "");
  const minutes = $derived(
    minute !== "" && !MINUTE_STEPS.includes(minute) ? [...MINUTE_STEPS, minute].sort() : MINUTE_STEPS,
  );
  const hourItems = HOURS.map((h) => ({ value: h, label: h }));
  const minuteItems = $derived(minutes.map((m) => ({ value: m, label: m })));

  function set(nextHour: string, nextMinute: string): void {
    value = `${nextHour || "00"}:${nextMinute || "00"}`;
  }
</script>

<div role="group" aria-label={label} class="flex items-center gap-1" dir="ltr">
  <Select.Root type="single" value={hour} onValueChange={(h) => set(h, minute)} items={hourItems}>
    <Select.Trigger aria-label={hourLabel} class="w-[4.5rem] tabular-nums">
      <Select.Value placeholder="--" />
    </Select.Trigger>
    <Select.Content class="max-h-64">
      {#each HOURS as h (h)}
        <Select.Item value={h} label={h}>{h}</Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>
  <span aria-hidden="true" class="text-muted">:</span>
  <Select.Root type="single" value={minute} onValueChange={(m) => set(hour, m)} items={minuteItems}>
    <Select.Trigger aria-label={minuteLabel} class="w-[4.5rem] tabular-nums">
      <Select.Value placeholder="--" />
    </Select.Trigger>
    <Select.Content class="max-h-64">
      {#each minutes as m (m)}
        <Select.Item value={m} label={m}>{m}</Select.Item>
      {/each}
    </Select.Content>
  </Select.Root>
</div>
