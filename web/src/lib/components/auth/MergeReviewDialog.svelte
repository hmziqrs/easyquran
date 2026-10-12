<script lang="ts">
  import { Dialog as DialogPrimitive } from "bits-ui";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import {
    auth_merge_apply_unowned,
    auth_merge_back,
    auth_merge_description,
    auth_merge_discard_unowned,
    auth_merge_done,
    auth_merge_error,
    auth_merge_merge,
    auth_merge_never_ask,
    auth_merge_not_now,
    auth_merge_outcome_account_wins,
    auth_merge_outcome_device_wins,
    auth_merge_outcome_new,
    auth_merge_result,
    auth_merge_result_only,
    auth_merge_review,
    auth_merge_session_row,
    auth_merge_session_state_row,
    auth_merge_sessions,
    auth_merge_summary,
    auth_merge_title,
    auth_merge_unowned,
  } from "#lib/i18n/m/auth.js";
  import { getLocale } from "#lib/paraglide/runtime.js";
  import type { UiLocale } from "#lib/i18n/locales.js";
  import { mergeReview, type MergeBookmarkRow } from "./merge-review.svelte";

  // §9.8 copy pass: catalog-driven strings (en + ar) replacing the English-only COPY block.
  const copy = $derived.by(() => {
    // SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale()
    // only ever returns a UiLocale at runtime.
    const locale = { locale: getLocale() as UiLocale };
    return {
      title: auth_merge_title(undefined, locale),
      description: auth_merge_description(undefined, locale),
      summary(count: number): string {
        return auth_merge_summary({ count }, locale);
      },
      sessions(count: number): string {
        return auth_merge_sessions({ count }, locale);
      },
      sessionRow: auth_merge_session_row(undefined, locale),
      sessionStateRow: auth_merge_session_state_row(undefined, locale),
      merge: auth_merge_merge(undefined, locale),
      review: auth_merge_review(undefined, locale),
      back: auth_merge_back(undefined, locale),
      notNow: auth_merge_not_now(undefined, locale),
      neverAsk: auth_merge_never_ask(undefined, locale),
      unowned(count: number): string {
        return auth_merge_unowned({ count }, locale);
      },
      applyUnowned: auth_merge_apply_unowned(undefined, locale),
      discardUnowned: auth_merge_discard_unowned(undefined, locale),
      outcome(row: MergeBookmarkRow): string {
        const outcome = mergeReview.outcomeFor(row);
        if (outcome === "new") return auth_merge_outcome_new(undefined, locale);
        if (outcome === "device-wins") return auth_merge_outcome_device_wins(undefined, locale);
        return auth_merge_outcome_account_wins(undefined, locale);
      },
      result(merged: number, skipped: number): string {
        if (skipped === 0) return auth_merge_result_only({ merged }, locale);
        return auth_merge_result({ merged, skipped }, locale);
      },
      done: auth_merge_done(undefined, locale),
      error: auth_merge_error(undefined, locale),
    };
  });

  let showList = $state(false);
  let excluded = $state<Record<string, boolean>>({});
  let excludedSessions = $state<Record<string, boolean>>({});

  const rows = $derived(mergeReview.rows);
  const chosen = $derived(rows.filter((row) => !excluded[row.key]));
  const sessionRows = $derived(mergeReview.sessionRows);
  const chosenSessions = $derived(sessionRows.filter((row) => !excludedSessions[row.id]));
  const hasMergeable = $derived(rows.length > 0 || sessionRows.length > 0);

  // Selection is per-open: every close path resets it (effect-free on purpose).
  function close(): void {
    excluded = {};
    excludedSessions = {};
    showList = false;
    mergeReview.close();
  }

  function onMerge(): void {
    void mergeReview.merge(
      chosen.map((row) => row.key),
      chosenSessions.map((row) => row.id),
    );
  }

  // "Don't ask again" rides the same reset path as every close: neverAsk()
  // marks first (so close() writes no deferred marker), then the excluded-rows
  // and review-step state reset — a later account's review starts clean.
  function onNeverAsk(): void {
    mergeReview.neverAsk();
    close();
  }
</script>

<DialogPrimitive.Root
  open={mergeReview.open}
  onOpenChange={(next) => {
    if (!next) close();
  }}
>
  <DialogPrimitive.Portal>
    <DialogPrimitive.Overlay
      data-slot="merge-review-overlay"
      class="fixed inset-0 z-[91] bg-black/55 supports-backdrop-filter:backdrop-blur-sm data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0"
    />
    <DialogPrimitive.Content
      data-slot="merge-review-dialog"
      class="fixed left-1/2 top-1/2 z-[92] w-[calc(100vw-2rem)] max-w-[440px] -translate-x-1/2 -translate-y-1/2 overflow-hidden rounded-xl border border-border bg-surface-raised outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95"
    >
      <DialogPrimitive.Title class="sr-only">{copy.title}</DialogPrimitive.Title>
      <DialogPrimitive.Description class="sr-only">{copy.description}</DialogPrimitive.Description>
      <DialogPrimitive.Close
        class="absolute end-3.5 top-3.5 z-10 inline-flex size-9 items-center justify-center rounded-pill text-muted transition-colors hover:bg-surface-hover hover:text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
        aria-label={copy.notNow}><Icon name="x" size={18} /></DialogPrimitive.Close
      >
      <div class="max-h-[82vh] overflow-y-auto px-6 py-7">
        <h2 class="text-h3 text-foreground">{copy.title}</h2>
        {#if mergeReview.result}
          {@const res = mergeReview.result}
          <p class="mt-2 text-body-s text-foreground-secondary" role="status">
            {copy.result(res.merged, res.skipped)}
          </p>
          <div class="mt-6 flex justify-end">
            <Button variant="primary" size="sm" onclick={() => close()}
              >{copy.done}</Button
            >
          </div>
        {:else}
          {#if rows.length > 0}
            <p class="mt-2 text-body-s text-foreground-secondary">{copy.summary(rows.length)}</p>
          {/if}
          {#if sessionRows.length > 0}
            <p class="mt-2 text-body-s text-foreground-secondary">{copy.sessions(sessionRows.length)}</p>
          {/if}
          {#if mergeReview.unowned > 0}
            <div class="mt-4 rounded-lg border border-border bg-surface p-3.5">
              <p class="text-caption text-foreground-secondary">{copy.unowned(mergeReview.unowned)}</p>
              <div class="mt-2.5 flex gap-2">
                <Button variant="secondary" size="sm" onclick={() => void mergeReview.applyUnowned()}
                  >{copy.applyUnowned}</Button
                >
                <Button variant="ghost" size="sm" onclick={() => void mergeReview.discardUnowned()}
                  >{copy.discardUnowned}</Button
                >
              </div>
            </div>
          {/if}
          {#if showList && hasMergeable}
            <ul class="mt-4 flex max-h-56 flex-col gap-1 overflow-y-auto">
              {#each rows as row (row.key)}
                <li class="flex items-start gap-2.5 rounded-md p-2 hover:bg-surface-hover">
                  <!-- Native checkbox restyled like TranslationRow: appearance-none
                       keeps native behaviour, tokens restyle box + check. -->
                  <span class="relative mt-0.5 flex size-[18px] flex-none items-center justify-center">
                    <input
                      id="merge-row-{row.key}"
                      type="checkbox"
                      checked={!excluded[row.key]}
                      onchange={(event) => {
                        excluded = { ...excluded, [row.key]: !event.currentTarget.checked };
                      }}
                      class="peer size-[18px] cursor-pointer appearance-none rounded-[5px] border-[1.5px] border-border-strong checked:border-primary checked:bg-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                    />
                    <Icon
                      name="check"
                      size={12}
                      class="pointer-events-none absolute text-primary-foreground opacity-0 peer-checked:opacity-100"
                    />
                  </span>
                  <label for="merge-row-{row.key}" class="flex flex-col gap-0.5">
                    <span class="text-body-s text-foreground">{row.key}</span>
                    <span class="text-caption text-foreground-secondary">{copy.outcome(row)}</span>
                  </label>
                </li>
              {/each}
              {#each sessionRows as row (row.id)}
                <li class="flex items-start gap-2.5 rounded-md p-2 hover:bg-surface-hover">
                  <!-- Native checkbox restyled like TranslationRow: appearance-none
                       keeps native behaviour, tokens restyle box + check. -->
                  <span class="relative mt-0.5 flex size-[18px] flex-none items-center justify-center">
                    <input
                      id="merge-session-{row.id}"
                      type="checkbox"
                      checked={!excludedSessions[row.id]}
                      onchange={(event) => {
                        excludedSessions = { ...excludedSessions, [row.id]: !event.currentTarget.checked };
                      }}
                      class="peer size-[18px] cursor-pointer appearance-none rounded-[5px] border-[1.5px] border-border-strong checked:border-primary checked:bg-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring"
                    />
                    <Icon
                      name="check"
                      size={12}
                      class="pointer-events-none absolute text-primary-foreground opacity-0 peer-checked:opacity-100"
                    />
                  </span>
                  <label for="merge-session-{row.id}" class="flex flex-col gap-0.5">
                    <span class="text-body-s text-foreground">{row.name}</span>
                    <span class="text-caption text-foreground-secondary"
                      >{row.isDefault ? copy.sessionStateRow : copy.sessionRow}</span
                    >
                  </label>
                </li>
              {/each}
            </ul>
          {/if}
          {#if mergeReview.error}
            <p role="alert" class="mt-4 text-caption text-danger">{copy.error}</p>
          {/if}
          <div class="mt-6 flex flex-wrap items-center justify-end gap-2">
            {#if hasMergeable}
              {#if showList}
                <Button variant="ghost" size="sm" onclick={() => (showList = false)}
                  >{copy.back}</Button
                >
              {:else}
                <Button variant="ghost" size="sm" onclick={() => (showList = true)}
                  >{copy.review}</Button
                >
              {/if}
            {/if}
            <Button variant="ghost" size="sm" onclick={onNeverAsk}>{copy.neverAsk}</Button>
            <Button variant="ghost" size="sm" onclick={() => close()}
              >{copy.notNow}</Button
            >
            {#if hasMergeable}
              <Button variant="primary" size="sm" disabled={mergeReview.merging} onclick={onMerge}
                >{copy.merge}</Button
              >
            {/if}
          </div>
        {/if}
      </div>
    </DialogPrimitive.Content>
  </DialogPrimitive.Portal>
</DialogPrimitive.Root>
