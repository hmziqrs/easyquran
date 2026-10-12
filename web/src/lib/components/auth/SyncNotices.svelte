<script lang="ts">
  import { untrack } from "svelte";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import {
    consumeSignOutNotice,
    signOutNoticeArmed,
  } from "#lib/auth/sign-out-notice.svelte.js";
  import {
    auth_session_expired_banner,
    auth_sign_in,
    auth_sign_out_unsynced_note,
  } from "#lib/i18n/m/auth.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { getLocale } from "#lib/paraglide/runtime.js";

  // Locale changes navigate to a localized URL (full reload), so resolving once at init is sound — same idiom as UpdateToast.
  // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
  const locale = getLocale() as UiLocale;
  const options = { locale } as const;

  const copy = {
    expired: auth_session_expired_banner(undefined, options),
    signIn: auth_sign_in(undefined, options),
    signOutNote: auth_sign_out_unsynced_note(undefined, options),
  };

  /** §9.5 (EC-15/16/19): stays until the next successful sign-in (auth-state clears it). */
  const expired = $derived(authState.sessionExpired);

  let showSignOutNote = $state(false);
  const SIGN_OUT_NOTE_MS = 8000;
  // Plain latch, not state: it only gates this effect's control flow. Needed
  // because consuming the armed flag writes the same signal this effect reads
  // (untrack covers read-tracking, not write-invalidation), which would re-run
  // the effect and let its cleanup kill the still-running timer.
  let noticed = false;

  // §9.5 (EC-14 notice): armed right before the account view tears down,
  // consumed exactly once here — the line shows briefly, then lapses.
  $effect(() => {
    const armed = signOutNoticeArmed();
    if (!armed) {
      noticed = false;
      return;
    }
    if (noticed) return;
    noticed = true;
    showSignOutNote = true;
    const timer = setTimeout(() => {
      showSignOutNote = false;
      untrack(() => consumeSignOutNotice());
    }, SIGN_OUT_NOTE_MS);
    return () => clearTimeout(timer);
  });

  function openSignIn(): void {
    authModal.show("login");
  }
</script>

{#if expired || showSignOutNote}
  <div
    role="status"
    aria-live="polite"
    class="fixed left-1/2 top-4 z-[90] flex w-[min(92vw,420px)] -translate-x-1/2 flex-col gap-2"
  >
    {#if expired}
      <div
        class="flex items-center justify-between gap-3 rounded-xl border border-border bg-surface/95 p-3.5 backdrop-blur"
      >
        <p class="text-body-s text-foreground">{copy.expired}</p>
        <button
          type="button"
          onclick={openSignIn}
          class="shrink-0 rounded-pill bg-primary px-2.5 py-1 text-caption font-medium text-primary-foreground transition-[filter] duration-150 hover:brightness-[0.96]"
        >
          {copy.signIn}
        </button>
      </div>
    {/if}
    {#if showSignOutNote}
      <p
        class="rounded-xl border border-border bg-surface/95 p-3.5 text-caption leading-relaxed text-foreground-secondary backdrop-blur"
      >
        {copy.signOutNote}
      </p>
    {/if}
  </div>
{/if}
