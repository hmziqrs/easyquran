<script lang="ts">
  import { notifications } from "$lib/stores/notifications.svelte";
  import { cn } from "$lib/utils";

  const pill = "rounded-md border px-3 py-1.5 text-xs transition-colors duration-150";

  async function toggle() {
    if (notifications.subscribed) await notifications.unsubscribe();
    else await notifications.subscribe();
  }

  function toggleLabel(): string {
    if (notifications.busy) return "Working…";
    if (notifications.subscribed) return "Disable";
    if (notifications.permission === "denied") return "Blocked";
    if (notifications.supported === false) return "Unsupported";
    return "Enable";
  }

  let disabled = $derived(!notifications.subscribed && !notifications.canSubscribe);
  let label = $derived(toggleLabel());
</script>

<div class="grid gap-1.5">
  <div class="flex items-center justify-between gap-2">
    <span class="text-xs text-fg-3">Notifications</span>
    <span class="text-right text-[11px] leading-tight text-fg-3">
      {notifications.statusText}
    </span>
  </div>
  <button
    type="button"
    {disabled}
    onclick={toggle}
    aria-pressed={notifications.subscribed}
    class={cn(
      pill,
      notifications.subscribed
        ? "border-line-2 text-fg-2 hover:text-fg"
        : "border-accent bg-accent-soft text-fg hover:opacity-90",
      disabled && "cursor-not-allowed opacity-50",
    )}
  >
    {label}
  </button>
</div>
