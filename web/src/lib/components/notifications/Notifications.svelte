<!--
  Notifications — the push opt-in/opt-out control rendered inside the Tweaks
  panel. It reads the notifications store (support, permission, subscription)
  and drives subscribe()/unsubscribe() on click. Subscribe runs the permission
  prompt, so it must originate from this user gesture (Safari/iOS web push
  requires it). Degrades gracefully when push is unsupported or blocked.
-->
<script lang="ts">
  import { notifications } from "$lib/stores/notifications.svelte";
  import { cn } from "$lib/utils";

  const pill = "rounded-md border px-3 py-1.5 text-xs transition-colors duration-150";

  async function toggle() {
    if (notifications.subscribed) await notifications.unsubscribe();
    else await notifications.subscribe();
  }

  // Disabled when there's nothing the user can do here (unsupported or blocked
  // and not currently subscribed).
  let disabled = $derived(!notifications.subscribed && !notifications.canSubscribe);
  let label = $derived(
    notifications.busy
      ? "Working…"
      : notifications.subscribed
        ? "Disable"
        : notifications.permission === "denied"
          ? "Blocked"
          : (notifications.supported === false ? "Unsupported" : "Enable"),
  );
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
