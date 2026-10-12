<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { Dialog as DialogPrimitive } from "bits-ui";
  import { Icon } from "#lib/components/icon/index.js";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { commandPalette } from "#lib/stores/command-palette.svelte.js";
  import { localeSuggest } from "#lib/i18n/locale-suggest.svelte.js";
  import { UI_LOCALES, uiDirection } from "#lib/i18n/locales.js";
  import {
    locale_suggest_accept,
    locale_suggest_body,
    locale_suggest_close,
    locale_suggest_dismiss,
    locale_suggest_title,
  } from "#lib/i18n/m/chrome.js";

  // Past LCP and the offline-engine kick (plan §5.3); only the initial evaluation waits.
  const INITIAL_DELAY_MS = 1500;

  const copy = $derived.by(() => {
    const target = localeSuggest.target;
    // The endonym placeholder ships pre-wrapped in Unicode isolates inside the message strings.
    const name = { locale: target === null ? "" : UI_LOCALES[target].endonym };
    return {
      title: locale_suggest_title(name, { locale: localeSuggest.current }),
      body: locale_suggest_body(name, { locale: localeSuggest.current }),
      accept: locale_suggest_accept(name, { locale: localeSuggest.current }),
      dismiss: locale_suggest_dismiss(undefined, { locale: localeSuggest.current }),
      close: locale_suggest_close(undefined, { locale: localeSuggest.current }),
      direction: uiDirection(localeSuggest.current),
    };
  });

  // Condition 6: force-close when a heavier modal opens on top; retry-once when it closes.
  $effect(() => {
    const busy = authModal.open || commandPalette.open;
    if (busy) untrack(() => localeSuggest.forceClose());
    else untrack(() => localeSuggest.retryIfArmed(location.pathname));
  });

  onMount(() => {
    localeSuggest.hydrate();
    let timer: ReturnType<typeof setTimeout> | null = null;
    let secondFrame = 0;
    // Own double-rAF: the root layout's firstPaintComplete is a closure sibling components can't read (§5.3).
    const firstFrame = requestAnimationFrame(() => {
      secondFrame = requestAnimationFrame(() => {
        timer = setTimeout(() => localeSuggest.evaluate(location.pathname), INITIAL_DELAY_MS);
      });
    });
    const offStorage = localeSuggest.subscribeExternal();
    return () => {
      cancelAnimationFrame(firstFrame);
      cancelAnimationFrame(secondFrame);
      if (timer !== null) clearTimeout(timer);
      offStorage();
    };
  });
</script>

{#if localeSuggest.open}
  <DialogPrimitive.Root open onOpenChange={(open) => localeSuggest.onDialogOpenChange(open)}>
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay
        data-slot="locale-suggest-overlay"
        class="fixed inset-0 z-[92] bg-black/55 supports-backdrop-filter:backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0"
      />
      <DialogPrimitive.Content
        data-slot="locale-suggest-dialog"
        lang={localeSuggest.current}
        dir={copy.direction}
        onEscapeKeydown={() => localeSuggest.noteCloseIntent("escape")}
        onInteractOutside={() => localeSuggest.noteCloseIntent("overlay")}
        class="fixed left-1/2 top-1/2 z-[93] w-[calc(100vw-2rem)] max-w-[400px] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-xl border border-border bg-surface-raised px-6 py-7 outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
      >
        <DialogPrimitive.Close
          onclick={() => localeSuggest.noteCloseIntent("close_icon")}
          class="absolute end-3.5 top-3.5 z-10 inline-flex size-9 items-center justify-center rounded-pill text-muted transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
          aria-label={copy.close}><Icon name="x" size={18} /></DialogPrimitive.Close
        >
        <DialogPrimitive.Title class="pe-9 text-[17px] font-medium tracking-[-0.02em] text-foreground"
          >{copy.title}</DialogPrimitive.Title
        >
        <DialogPrimitive.Description class="mt-1 text-[14.5px] leading-relaxed text-foreground-secondary"
          >{copy.body}</DialogPrimitive.Description
        >
        <div class="mt-5 flex flex-wrap justify-end gap-2">
          <button
            type="button"
            onclick={() => localeSuggest.dismiss("button")}
            class="rounded-pill border border-border-strong px-3.5 py-2 text-caption text-foreground-secondary transition-colors duration-150 hover:border-border hover:text-foreground"
            >{copy.dismiss}</button
          >
          <a
            href={localeSuggest.href}
            hreflang={localeSuggest.target ?? undefined}
            lang={localeSuggest.target ?? undefined}
            data-sveltekit-reload
            onclick={() => localeSuggest.accept()}
            class="rounded-pill bg-primary px-3.5 py-2 text-caption font-medium text-primary-foreground transition-colors duration-150 hover:bg-primary-hover"
            >{copy.accept}</a
          >
        </div>
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  </DialogPrimitive.Root>
{/if}
