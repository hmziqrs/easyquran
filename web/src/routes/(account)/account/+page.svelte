<script lang="ts">
  import { goto } from "$app/navigation";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { createLogoutFlow } from "$lib/auth/flows.svelte";
  import { createPasskeyFlow } from "$lib/auth/passkey-flow.svelte";
  import { accountClient, type SessionInfo } from "$lib/auth/account-client";
  import { GENERIC_TRY_AGAIN } from "$lib/auth/auth-copy";
  import TwoFactorControls from "$lib/auth/components/TwoFactorControls.svelte";
  import {
    resolveAccountView,
    sessionDeviceLabel,
    ANONYMOUS_CTA_HREF,
    REGISTER_HREF,
  } from "./account-view";

  const view = $derived(resolveAccountView(authState.status));
  const user = $derived(authState.user);

  const logout = createLogoutFlow();
  const passkey = createPasskeyFlow();

  let sessions = $state<SessionInfo[]>([]);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);

  let nameDraft = $state("");
  let profilePending = $state(false);
  let profileError = $state<string | null>(null);
  let profileSaved = $state(false);

  let terminatePendingId = $state<string | null>(null);
  let terminateError = $state<string | null>(null);

  let initialized = false;

  $effect(() => {
    if (view !== "authenticated" || initialized) return;
    initialized = true;
    if (user) nameDraft = user.name;
    void loadSessions();
    void passkey.list();
  });

  async function loadSessions(): Promise<void> {
    sessionsLoading = true;
    sessionsError = null;
    const res = await accountClient.listSessions();
    sessionsLoading = false;
    if (res.status === "ok" && res.data) {
      sessions = res.data;
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      sessionsError = GENERIC_TRY_AGAIN;
    }
  }

  async function saveProfile(): Promise<void> {
    if (profilePending || !user) return;
    const name = nameDraft.trim();
    if (!name) {
      profileError = "Name can't be empty.";
      return;
    }
    profilePending = true;
    profileError = null;
    profileSaved = false;
    const res = await accountClient.updateProfile({ name });
    profilePending = false;
    if (res.status === "ok" && res.data) {
      authState.setUser(res.data);
      profileSaved = true;
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      profileError = GENERIC_TRY_AGAIN;
    }
  }

  async function terminate(id: string, isCurrent: boolean): Promise<void> {
    if (terminatePendingId) return;
    terminatePendingId = id;
    terminateError = null;
    const res = await accountClient.terminateSession(id);
    terminatePendingId = null;
    if (res.status === "ok") {
      if (res.isCurrent || isCurrent) {
        await logout.run();
        await goto(ANONYMOUS_CTA_HREF);
      } else {
        await loadSessions();
      }
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      terminateError = GENERIC_TRY_AGAIN;
    }
  }

  async function doLogout(): Promise<void> {
    const ok = await logout.run();
    if (ok) await goto(ANONYMOUS_CTA_HREF);
  }
</script>

<svelte:head>
  <title>Account</title>
</svelte:head>

{#if view === "loading"}
  <div
    class="flex flex-col items-center gap-4 py-12 text-center"
    role="status"
    aria-live="polite"
  >
    <span
      class="h-8 w-8 animate-spin rounded-full border-2 border-line-2 border-t-accent"
      aria-hidden="true"></span>
    <div class="flex flex-col gap-1">
      <h1 class="text-xl font-semibold">Loading your account…</h1>
      <p class="text-sm text-fg-2">Checking your session.</p>
    </div>
    <Button variant="ghost" size="sm" onclick={() => authState.probe()}>Retry</Button>
  </div>
{:else if view === "anonymous"}
  <section class="mx-auto flex w-full max-w-[420px] flex-col gap-5">
    <div class="flex flex-col gap-2">
      <h1 class="text-[28px] font-semibold leading-tight tracking-[-0.02em]">Your account</h1>
      <p class="text-[15px] leading-relaxed text-fg-2">
        Sign in to manage your profile, sessions, and security.
      </p>
    </div>
    <Button variant="accent" size="lg" onclick={() => goto(ANONYMOUS_CTA_HREF)}>Sign in</Button>
    <p class="text-center text-sm text-fg-2">
      No account? <a href={REGISTER_HREF} class="text-accent hover:underline">Create one</a>
    </p>
  </section>
{:else}
  <div class="flex flex-col gap-10">
    <div class="flex items-start justify-between gap-4">
      <div class="flex flex-col gap-1">
        <h1 class="text-[28px] font-semibold leading-tight tracking-[-0.02em]">Your account</h1>
        <p class="text-sm text-fg-2">Manage your profile, sessions, and security.</p>
      </div>
      <Button variant="ghost" size="sm" disabled={logout.pending} onclick={doLogout}>
        {logout.pending ? "Signing out…" : "Sign out"}
      </Button>
    </div>

    <section class="flex flex-col gap-4">
      <h2 class="text-lg font-semibold">Profile</h2>
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1.5">
          <Label for="account-name">Name</Label>
          <Input
            id="account-name"
            type="text"
            autocomplete="name"
            bind:value={nameDraft}
            disabled={profilePending}
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <Label for="account-email">Email</Label>
          <Input id="account-email" type="email" value={user?.email ?? ""} disabled />
        </div>
        {#if profileError}
          <p role="alert" aria-live="assertive" class="text-sm text-destructive">{profileError}</p>
        {/if}
        {#if profileSaved}
          <p role="status" aria-live="polite" class="text-sm text-accent">Profile updated.</p>
        {/if}
        <Button
          variant="accent"
          class="self-start"
          disabled={profilePending}
          onclick={saveProfile}
        >
          {profilePending ? "Saving…" : "Save changes"}
        </Button>
      </div>
    </section>

    <section class="flex flex-col gap-4">
      <div class="flex flex-col gap-1">
        <h2 class="text-lg font-semibold">Sessions</h2>
        <p class="text-sm text-fg-2">Devices currently signed in to your account.</p>
      </div>
      {#if sessionsError}
        <p role="alert" aria-live="assertive" class="text-sm text-destructive">{sessionsError}</p>
        <Button variant="ghost" size="sm" onclick={loadSessions}>Retry</Button>
      {:else if sessionsLoading && sessions.length === 0}
        <p class="text-sm text-fg-2" role="status" aria-live="polite">Loading sessions…</p>
      {:else if sessions.length === 0}
        <p class="text-sm text-fg-2">No sessions found.</p>
      {:else}
        <ul class="flex flex-col gap-2">
          {#each sessions as s (s.id)}
            <li
              class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-line-2 bg-bg-2 px-4 py-3"
            >
              <div class="flex flex-col gap-0.5">
                <span class="text-sm font-medium">
                  {sessionDeviceLabel(s.userAgent)}
                  {#if s.isCurrent}
                    <span
                      class="ml-2 rounded-full bg-accent-soft px-2 py-0.5 text-xs text-accent"
                    >This session</span>
                  {/if}
                </span>
                {#if s.lastSeenAt}
                  <span class="text-xs text-fg-3">Last seen {s.lastSeenAt}</span>
                {/if}
              </div>
              <Button
                variant="ghost"
                size="sm"
                disabled={terminatePendingId === s.id}
                onclick={() => terminate(s.id, s.isCurrent)}
              >
                {terminatePendingId === s.id ? "Ending…" : s.isCurrent ? "Sign out" : "Revoke"}
              </Button>
            </li>
          {/each}
        </ul>
        {#if terminateError}
          <p role="alert" aria-live="assertive" class="text-sm text-destructive">{terminateError}</p>
        {/if}
      {/if}
    </section>

    <section class="flex flex-col gap-4">
      <h2 class="text-lg font-semibold">Security</h2>
      <div class="flex flex-col gap-5">
        <TwoFactorControls />

        <div class="flex flex-col gap-3 rounded-xl border border-line-2 bg-bg-2 p-5">
          <div class="flex flex-col gap-1">
            <h3 class="text-base font-semibold">Passkeys</h3>
            <p class="text-sm text-fg-2">Sign in without a password using a passkey.</p>
          </div>
          {#if !passkey.supported}
            <p class="text-sm text-fg-2">Passkeys aren't supported on this device.</p>
          {:else}
            {#if passkey.passkeys.length > 0}
              <ul class="flex flex-col gap-2">
                {#each passkey.passkeys as pk (pk.id)}
                  <li class="flex items-center justify-between gap-3">
                    <span class="text-sm">{pk.label || "Passkey"}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={passkey.pending}
                      onclick={() => passkey.remove(pk.id)}
                    >
                      Remove
                    </Button>
                  </li>
                {/each}
              </ul>
            {/if}
            {#if passkey.genericError}
              <p role="alert" aria-live="assertive" class="text-sm text-destructive">
                {passkey.genericError}
              </p>
            {/if}
            <Button
              variant="accent"
              class="self-start"
              disabled={passkey.pending}
              onclick={() => passkey.register()}
            >
              {passkey.pending ? "Adding…" : "Add a passkey"}
            </Button>
          {/if}
        </div>
      </div>
    </section>
  </div>
{/if}
