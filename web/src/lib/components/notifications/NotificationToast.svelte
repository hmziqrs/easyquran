<script lang="ts">
  import { notifications } from "$lib/stores/notifications.svelte";
  import type { Attachment } from "svelte/attachments";

  function dismiss(): void {
    notifications.clearMessage();
  }

  const autoDismiss: Attachment<HTMLElement> = (node) => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const arm = () => {
      timer = setTimeout(dismiss, 7000);
    };
    const pause = () => {
      if (timer) {
        clearTimeout(timer);
        timer = null;
      }
    };
    node.addEventListener("focusin", pause);
    node.addEventListener("focusout", arm);
    arm();
    return () => {
      pause();
      node.removeEventListener("focusin", pause);
      node.removeEventListener("focusout", arm);
    };
  };
</script>

{#if notifications.lastMessage}
  {#key notifications.messageSeq}
    {@const n = notifications.lastMessage.notification}
    {@const data = notifications.lastMessage.data ?? {}}
    {@const title = n?.title ?? "EasyQuran"}
    {@const body = n?.body ?? ""}
    {@const url = typeof data.url === "string" ? data.url : undefined}

    <div
      {@attach autoDismiss}
      role="status"
      aria-live="polite"
      class="fixed left-1/2 top-4 z-[1001] flex w-[min(92vw,380px)] -translate-x-1/2 items-start gap-3 rounded-xl border border-border-strong bg-surface/95 p-3.5 backdrop-blur"
    >
      <div class="flex size-8 shrink-0 items-center justify-center rounded-sm bg-primary-soft text-primary">
        <span class="text-base leading-none" aria-hidden="true">✦</span>
      </div>
      <div class="min-w-0 flex-1">
        <p class="truncate text-sm font-medium text-foreground">{title}</p>
        {#if body}
          <p class="mt-0.5 line-clamp-2 text-xs leading-relaxed text-foreground-secondary">{body}</p>
        {/if}
        {#if url}
          <a
            href={url}
            onclick={dismiss}
            class="mt-1.5 inline-block text-xs text-primary underline underline-offset-2 hover:text-primary/80"
          >
            Open
          </a>
        {/if}
      </div>
      <button
        type="button"
        onclick={dismiss}
        aria-label="Dismiss notification"
        class="shrink-0 text-muted transition-colors hover:text-foreground">✕</button
      >
    </div>
  {/key}
{/if}
