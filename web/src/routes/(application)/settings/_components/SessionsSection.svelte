<script lang="ts">
  import { onMount } from "svelte";

  import { sessions } from "#lib/sessions/store.svelte.js";
  import {
    SESSION_NAME_MAX,
    sessionBucketFor,
    type ReadingSession,
    type SessionKind,
  } from "#lib/sessions/schema.svelte.js";
  import { cn } from "#lib/utils.js";
  import {
    reader_bookmarks_cancel,
    reader_bookmarks_rename,
    reader_bookmarks_save,
    reader_sessions_archive,
    reader_sessions_archived,
    reader_sessions_bucket_day,
    reader_sessions_bucket_week,
    reader_sessions_delete,
    reader_sessions_delete_confirm,
    reader_sessions_kind,
    reader_sessions_kind_daily,
    reader_sessions_kind_free,
    reader_sessions_kind_shared,
    reader_sessions_kind_weekly,
    reader_sessions_label,
    reader_sessions_new_add,
    reader_sessions_new_placeholder,
    reader_sessions_restore,
  } from "#lib/i18n/m/reader.js";

  // Feature 4.5: Reading-level session controls — switch, rename,
  // archive/restore, delete, create daily/weekly. Device-local-first store:
  // rows render only after mount (SSR has no partition).
  let mounted = $state(false);

  const live = $derived(sessions.list.filter((session) => session.archivedAt === null));
  const archived = $derived(sessions.list.filter((session) => session.archivedAt !== null));
  const activeSessionId = $derived(sessions.activeId ?? "");

  /** Daily/weekly are creatable; `shared` stays a display-only seam (Feature 4.1). */
  const CREATABLE_KINDS: readonly SessionKind[] = ["free", "daily", "weekly"];

  let newName = $state("");
  let newKind = $state<SessionKind>("free");
  let renamingId = $state<string | null>(null);
  let renameValue = $state("");
  let confirmingId = $state<string | null>(null);

  onMount(() => {
    mounted = true;
  });

  function kindLabel(kind: SessionKind): string {
    if (kind === "daily") return reader_sessions_kind_daily();
    if (kind === "weekly") return reader_sessions_kind_weekly();
    if (kind === "shared") return reader_sessions_kind_shared();
    return reader_sessions_kind_free();
  }

  /**
   * Kind + derived bucket caption (Feature 4.8, EC-36): daily/weekly buckets
   * come from the device-local day/week at read time and are never stored.
   */
  function rowCaption(session: ReadingSession): string {
    const kind = kindLabel(session.kind);
    if (session.kind === "daily") {
      return `${kind} · ${reader_sessions_bucket_day()} · ${sessionBucketFor("daily")}`;
    }
    if (session.kind === "weekly") {
      return `${kind} · ${reader_sessions_bucket_week()} · ${sessionBucketFor("weekly")}`;
    }
    return kind;
  }

  function addSession(): void {
    const created = sessions.create(newName, newKind);
    if (created !== null) newName = "";
  }

  function startRename(session: ReadingSession): void {
    confirmingId = null;
    renamingId = session.id;
    renameValue = session.name;
  }

  function saveRename(id: string): void {
    sessions.update(id, { name: renameValue });
    renamingId = null;
  }

  const fieldClass =
    "h-9 w-full rounded-pill border border-border bg-transparent px-3 text-sm text-foreground placeholder:text-muted focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
  const actionButton =
    "flex h-8 flex-none items-center rounded-pill border border-border px-2.5 text-caption font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground disabled:opacity-40 disabled:pointer-events-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring";
  const dangerButton = cn(actionButton, "border-danger text-danger hover:border-danger hover:text-danger");
</script>

<section class="mt-10" aria-labelledby="settings-sessions-heading">
  <h2 id="settings-sessions-heading" class="text-[17px] font-medium tracking-[-0.02em] text-foreground">
    {reader_sessions_label()}
  </h2>

  {#if mounted}
    <div class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
      <div class="px-4 py-3.5 sm:px-5">
        <label class="flex max-w-md flex-col gap-1.5">
          <span class="text-[14.5px] font-medium text-foreground">{reader_sessions_label()}</span>
          <select
            aria-label={reader_sessions_label()}
            value={activeSessionId}
            onchange={(event) => sessions.switchTo(event.currentTarget.value)}
            class={fieldClass}>
            {#each live as session (session.id)}
              <option value={session.id}>{session.name}</option>
            {/each}
          </select>
        </label>
      </div>

      {#each live as session (session.id)}
        <div class="flex flex-wrap items-center gap-2 px-4 py-3 sm:px-5">
          {#if renamingId === session.id}
            <form
              class="flex min-w-0 flex-1 flex-wrap items-center gap-2"
              onsubmit={(event) => {
                event.preventDefault();
                saveRename(session.id);
              }}>
              <input
                type="text"
                bind:value={renameValue}
                maxlength={SESSION_NAME_MAX}
                aria-label={reader_bookmarks_rename()}
                class={cn(fieldClass, "min-w-0 flex-1 sm:max-w-[260px]")} />
              <button type="submit" class={actionButton} disabled={renameValue.trim().length === 0}>
                {reader_bookmarks_save()}
              </button>
              <button type="button" class={actionButton} onclick={() => (renamingId = null)}>
                {reader_bookmarks_cancel()}
              </button>
            </form>
          {:else if confirmingId === session.id}
            <span class="min-w-0 flex-1 text-caption text-foreground-secondary">
              {reader_sessions_delete_confirm({ name: session.name })}
            </span>
            <button
              type="button"
              class={dangerButton}
              onclick={() => sessions.remove(session.id)}>{reader_sessions_delete()}</button>
            <button
              type="button"
              class={actionButton}
              onclick={() => (confirmingId = null)}>{reader_bookmarks_cancel()}</button>
          {:else}
            <span class="min-w-0 flex-1">
              <span class="block truncate text-[14.5px] font-medium text-foreground">
                {session.name}
              </span>
              <span class="mt-0.5 block text-caption text-muted">{rowCaption(session)}</span>
            </span>
            <button type="button" class={actionButton} onclick={() => startRename(session)}>{reader_bookmarks_rename()}</button>
            <button
              type="button"
              class={actionButton}
              onclick={() => sessions.update(session.id, { archived: true })}>{reader_sessions_archive()}</button>
            <button
              type="button"
              class={dangerButton}
              onclick={() => (confirmingId = session.id)}>{reader_sessions_delete()}</button>
          {/if}
        </div>
      {/each}

      {#if archived.length > 0}
        <div class="px-4 py-3.5 sm:px-5">
          <span class="text-[14.5px] font-medium text-foreground">{reader_sessions_archived()}</span>
          <ul class="mt-2 grid gap-1.5">
            {#each archived as session (session.id)}
              <li class="flex flex-wrap items-center gap-2">
                <span class="min-w-0 flex-1 truncate text-[14.5px] text-foreground-secondary">
                  {session.name}
                  <span class="text-caption text-muted">· {rowCaption(session)}</span>
                </span>
                {#if confirmingId === session.id}
                  <span class="min-w-0 flex-1 text-caption text-foreground-secondary">
                    {reader_sessions_delete_confirm({ name: session.name })}
                  </span>
                  <button type="button" class={dangerButton} onclick={() => sessions.remove(session.id)}>
                    {reader_sessions_delete()}
                  </button>
                  <button type="button" class={actionButton} onclick={() => (confirmingId = null)}>
                    {reader_bookmarks_cancel()}
                  </button>
                {:else}
                  <button
                    type="button"
                    class={actionButton}
                    onclick={() => sessions.update(session.id, { archived: false })}>{reader_sessions_restore()}</button>
                  <button type="button" class={dangerButton} onclick={() => (confirmingId = session.id)}>
                    {reader_sessions_delete()}
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      <div class="px-4 py-3.5 sm:px-5">
        <form
          class="flex flex-wrap items-end gap-2"
          onsubmit={(event) => {
            event.preventDefault();
            addSession();
          }}>
          <label class="flex min-w-0 flex-1 flex-col gap-1.5 sm:max-w-[260px]">
            <span class="text-caption text-muted">{reader_sessions_new_placeholder()}</span>
            <input
              type="text"
              bind:value={newName}
              maxlength={SESSION_NAME_MAX}
              placeholder={reader_sessions_new_placeholder()}
              class={fieldClass} />
          </label>
          <label class="flex flex-col gap-1.5">
            <span class="text-caption text-muted">{reader_sessions_kind()}</span>
            <select bind:value={newKind} aria-label={reader_sessions_kind()} class={fieldClass}>
              {#each CREATABLE_KINDS as kind (kind)}
                <option value={kind}>{kindLabel(kind)}</option>
              {/each}
            </select>
          </label>
          <button
            type="submit"
            disabled={newName.trim().length === 0}
            class="h-9 flex-none rounded-pill border border-border px-3 text-caption font-medium text-foreground-secondary transition-colors hover:border-border-strong hover:text-foreground disabled:opacity-40 disabled:pointer-events-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus-ring">{reader_sessions_new_add()}</button>
        </form>
      </div>
    </div>
  {/if}
</section>
