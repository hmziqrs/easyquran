<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import { toast } from "svelte-sonner";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Card } from "#lib/components/ui/card/index.js";
  import { Icon } from "#lib/components/icon/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { authModal } from "#lib/auth/auth-modal.svelte.js";
  import { createLogoutFlow } from "#lib/auth/flows.svelte.js";
  import { createPasskeyFlow } from "#lib/auth/passkey-flow.svelte.js";
  import {
    accountClient,
    type DeviceInfo,
    type SessionInfo,
  } from "#lib/auth/account-client.js";
  import { consumeLinkProvider } from "#lib/auth/link-attempt.js";
  import { deviceIdFor } from "#lib/device/identity.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";
  import { getAccountCopy } from "#lib/i18n/account-copy.js";
  import { createClock, localClockStorage } from "#lib/sync/clock.js";
  import FreshAuthControls from "#lib/auth/components/FreshAuthControls.svelte";
  import TwoFactorControls from "#lib/auth/components/TwoFactorControls.svelte";
  import ConnectedAccounts from "#lib/auth/components/ConnectedAccounts.svelte";
  import DeleteAccountControls from "#lib/auth/components/DeleteAccountControls.svelte";
  import {
    resolveAccountView,
    resolveLinkNotice,
    deviceIconName,
    deviceTitle,
    deviceTypeLabel,
    normaliseDeviceNameDraft,
    sessionDeviceLabel,
    sessionLastSeenLabel,
    ANONYMOUS_CTA_HREF,
    type LinkNotice,
  } from "./account-view";

  // The link-notice helper still speaks the auth copy shape (provider banners live there).
  const authCopy = getAuthCopy();
  const copy = getAccountCopy();

  const view = $derived(resolveAccountView(authState.status));
  const user = $derived(authState.user);

  const logout = createLogoutFlow();
  const passkey = createPasskeyFlow();
  // Passkey membership changes flip the connected-accounts last-method gate.
  const passkeyKey = $derived(passkey.passkeys.map((pk) => pk.id).join(","));

  let sessions = $state<SessionInfo[]>([]);
  let sessionsLoading = $state(false);
  let sessionsError = $state<string | null>(null);

  let devices = $state<DeviceInfo[]>([]);
  let devicesLoading = $state(false);
  let devicesError = $state<string | null>(null);
  let devicePendingId = $state<string | null>(null);
  let renameId = $state<string | null>(null);
  let renameDraft = $state("");
  let renameError = $state<string | null>(null);
  let renamePending = $state(false);

  // A session bound to a listed device shows under that device. Any other session has
  // no device the server can name, so it shows as an unknown-device row.
  const listedDeviceIds = $derived(new Set(devices.map((device) => device.id)));
  const unboundSessions = $derived(
    sessions.filter((s) => s.deviceId === undefined || !listedDeviceIds.has(s.deviceId)),
  );
  const hasRows = $derived(devices.length > 0 || unboundSessions.length > 0);

  let nameDraft = $state("");
  let profilePending = $state(false);
  let profileError = $state<string | null>(null);
  let profileSaved = $state(false);

  let terminatePendingId = $state<string | null>(null);
  let terminateError = $state<string | null>(null);

  let linkNotice = $state<LinkNotice | null>(null);

  let initialized = false;

  onMount(() => {
    const params = page.url.searchParams;
    if (!params.has("linked") && !params.has("link_error")) return;
    linkNotice = resolveLinkNotice(params, authCopy, consumeLinkProvider());
    const url = new URL(page.url.href);
    url.searchParams.delete("linked");
    url.searchParams.delete("link_error");
    void goto(url, { replace: true, reset: false });
  });

  $effect(() => {
    if (view !== "authenticated" || initialized) return;
    initialized = true;
    if (user) nameDraft = user.name;
    void loadDevices();
    void loadSessions();
    void passkey.list();
  });

  async function loadDevices(): Promise<void> {
    devicesLoading = true;
    devicesError = null;
    const res = await accountClient.listDevices();
    devicesLoading = false;
    if (res.status === "ok" && res.data) {
      devices = res.data;
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      devicesError = copy.genericError;
    }
  }

  async function refreshAll(): Promise<void> {
    await Promise.all([loadDevices(), loadSessions()]);
  }

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
      sessionsError = copy.genericError;
    }
  }

  async function saveProfile(): Promise<void> {
    if (profilePending || !user) return;
    const name = nameDraft.trim();
    if (!name) {
      profileError = copy.nameEmpty;
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
      profileError = copy.genericError;
    }
  }

  async function terminate(id: string, isCurrent: boolean): Promise<void> {
    if (terminatePendingId) return;
    terminatePendingId = id;
    terminateError = null;
    const res = await accountClient.terminateSession(id);
    terminatePendingId = null;
    if (res.status === "ok") {
      if (isCurrent) {
        await logout.run();
        await goto(ANONYMOUS_CTA_HREF);
      } else {
        await refreshAll();
      }
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      terminateError = copy.genericError;
    }
  }

  /** Remote sign-out (devices.md D4): acts at once, with a toast and no dialog. */
  async function signOutDevice(device: DeviceInfo): Promise<void> {
    if (devicePendingId || !user) return;
    devicePendingId = device.id;
    const res = await accountClient.revokeDevice(user.id, device.id);
    devicePendingId = null;
    if (res.status === "ok") {
      toast.success(copy.deviceSignedOut(deviceTitle(device, copy.deviceUnknown)));
      await refreshAll();
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      toast.error(copy.genericError);
    }
  }

  function startRename(device: DeviceInfo): void {
    renameId = device.id;
    renameDraft = device.displayName ?? "";
    renameError = null;
  }

  function cancelRename(): void {
    renameId = null;
    renameError = null;
  }

  // Stamps the rename from this install's clock, so a newer name always wins (contracts/sync.md).
  function deviceNameStamp(owner: number): string {
    const key = String(owner);
    return createClock(deviceIdFor(key), localClockStorage(key)).stamp();
  }

  async function saveRename(device: DeviceInfo): Promise<void> {
    if (renamePending || !user) return;
    const name = normaliseDeviceNameDraft(renameDraft);
    if (name === null) {
      renameError = copy.deviceNameInvalid;
      return;
    }
    renamePending = true;
    renameError = null;
    const res = await accountClient.renameDevice(
      user.id,
      device.id,
      name,
      deviceNameStamp(user.id),
    );
    renamePending = false;
    if (res.status === "ok") {
      renameId = null;
      await loadDevices();
    } else if (res.status === "anonymous") {
      await goto(ANONYMOUS_CTA_HREF);
    } else {
      renameError = copy.genericError;
    }
  }

  async function doLogout(): Promise<void> {
    const ok = await logout.run();
    if (ok) await goto(ANONYMOUS_CTA_HREF);
  }

  function sessionActionLabel(session: SessionInfo, pendingId: string | null): string {
    if (pendingId === session.id) return copy.sessionEnding;
    if (session.isCurrent) return copy.signOut;
    return copy.sessionRevoke;
  }

  async function handleAnonymous(): Promise<void> {
    await goto(ANONYMOUS_CTA_HREF);
  }
</script>

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
      <h1 class="text-xl font-medium">{copy.loadingHeading}</h1>
      <p class="text-sm text-fg-2">{copy.loadingBody}</p>
    </div>
    <Button variant="ghost" size="sm" onclick={() => authState.probe()}>{copy.retry}</Button>
  </div>
{:else if view === "anonymous"}
  <section class="mx-auto flex w-full max-w-[420px] flex-col gap-6">
    <div class="flex flex-col gap-2">
      <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em]">{copy.heading}</h1>
      <p class="text-[15px] leading-relaxed text-fg-2">{copy.anonymousBody}</p>
    </div>
    <div class="flex flex-col gap-3 sm:flex-row">
      <Button variant="accent" size="lg" onclick={() => authModal.show("login")}>
        {copy.signIn}
      </Button>
      <Button variant="secondary" size="lg" onclick={() => authModal.show("register")}>
        {copy.createAccount}
      </Button>
    </div>
  </section>
{:else}
  <div class="flex flex-col gap-10">
    <div class="flex items-start justify-between gap-4">
      <div class="flex flex-col gap-1">
        <h1 class="text-[28px] font-medium leading-tight tracking-[-0.02em]">{copy.heading}</h1>
        <p class="text-sm text-fg-2">{copy.signedInBody}</p>
      </div>
      <Button variant="ghost" size="sm" disabled={logout.pending} onclick={doLogout}>
        {logout.pending ? copy.signingOut : copy.signOut}
      </Button>
    </div>

    {#if linkNotice?.kind === "success"}
      <p role="status" aria-live="polite" class="text-sm text-accent">{linkNotice.text}</p>
    {:else if linkNotice}
      <p role="alert" aria-live="assertive" class="text-sm text-destructive">{linkNotice.text}</p>
    {/if}

    <section class="flex flex-col gap-4">
      <h2 class="text-lg font-medium">{copy.profileHeading}</h2>
      <div class="flex flex-col gap-3">
        <div class="flex flex-col gap-1.5">
          <Label for="account-name">{copy.fieldName}</Label>
          <Input
            id="account-name"
            type="text"
            autocomplete="name"
            bind:value={nameDraft}
            disabled={profilePending}
          />
        </div>
        <div class="flex flex-col gap-1.5">
          <Label for="account-email">{copy.fieldEmail}</Label>
          <Input id="account-email" type="email" value={user?.email ?? ""} disabled />
        </div>
        {#if profileError}
          <p role="alert" aria-live="assertive" class="text-sm text-destructive">{profileError}</p>
        {/if}
        {#if profileSaved}
          <p role="status" aria-live="polite" class="text-sm text-accent">{copy.profileSaved}</p>
        {/if}
        <Button
          variant="accent"
          class="self-start"
          disabled={profilePending}
          onclick={saveProfile}
        >
          {profilePending ? copy.saving : copy.saveChanges}
        </Button>
      </div>
    </section>

    <section class="flex flex-col gap-4">
      <div class="flex flex-col gap-1">
        <h2 class="text-lg font-medium">{copy.devicesHeading}</h2>
        <p class="text-sm text-fg-2">{copy.devicesDescription}</p>
      </div>
      {#if devicesError}
        <p role="alert" aria-live="assertive" class="text-sm text-destructive">{devicesError}</p>
        <Button variant="ghost" size="sm" onclick={refreshAll}>{copy.retry}</Button>
      {/if}
      {#if sessionsError}
        <p role="alert" aria-live="assertive" class="text-sm text-destructive">{sessionsError}</p>
        <Button variant="ghost" size="sm" onclick={loadSessions}>{copy.retry}</Button>
      {/if}
      {#if !hasRows}
        {#if devicesLoading || sessionsLoading}
          <p class="text-sm text-fg-2" role="status" aria-live="polite">{copy.devicesLoading}</p>
        {:else if !devicesError && !sessionsError}
          <p class="text-sm text-fg-2">{copy.sessionsEmpty}</p>
        {/if}
      {:else}
        <ul class="flex flex-col gap-2">
          {#each devices as device (device.id)}
            {@const title = deviceTitle(device, copy.deviceUnknown)}
            {@const typeLabel = deviceTypeLabel(device, copy.deviceUnknown)}
            {@const lastSeen = sessionLastSeenLabel(device.lastSeenAt, copy.locale)}
            <li>
              <Card size="sm" class="flex-row flex-wrap items-center justify-between gap-3 px-4">
                {#if renameId === device.id}
                  <form
                    class="flex w-full flex-col gap-2"
                    onsubmit={(event: SubmitEvent) => {
                      event.preventDefault();
                      void saveRename(device);
                    }}
                  >
                    <Label for="device-name-{device.id}">{copy.deviceRenameLabel}</Label>
                    <Input
                      id="device-name-{device.id}"
                      type="text"
                      maxlength={40}
                      autocomplete="off"
                      bind:value={renameDraft}
                      disabled={renamePending}
                    />
                    {#if renameError}
                      <p role="alert" aria-live="assertive" class="text-sm text-destructive">
                        {renameError}
                      </p>
                    {/if}
                    <div class="flex flex-wrap gap-2">
                      <Button variant="accent" size="sm" type="submit" disabled={renamePending}>
                        {copy.deviceRenameSave}
                      </Button>
                      <Button variant="ghost" size="sm" type="button" onclick={cancelRename}>
                        {copy.deviceRenameCancel}
                      </Button>
                    </div>
                  </form>
                {:else}
                  <div class="flex min-w-0 items-start gap-3">
                    <Icon name={deviceIconName(device)} size={18} class="mt-0.5 flex-none text-fg-3" />
                    <div class="flex min-w-0 flex-col gap-0.5">
                      <span class="text-sm font-medium">
                        {title}
                        {#if device.isCurrent}
                          <span
                            class="ms-2 rounded-pill bg-accent-soft px-2 py-0.5 text-xs text-accent"
                          >{copy.deviceThis}</span>
                        {/if}
                      </span>
                      {#if device.displayName !== null}
                        <span class="text-xs text-fg-2">{typeLabel}</span>
                      {/if}
                      {#if lastSeen}
                        <span class="text-xs text-fg-3">{copy.sessionLastSeen(lastSeen)}</span>
                      {/if}
                    </div>
                  </div>
                  <div class="flex items-center gap-1">
                    <Button variant="ghost" size="sm" onclick={() => startRename(device)}>
                      {copy.deviceRename}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={devicePendingId === device.id}
                      onclick={() => (device.isCurrent ? doLogout() : signOutDevice(device))}
                    >
                      {#if devicePendingId === device.id}
                        {copy.signingOut}
                      {:else if device.isCurrent}
                        {copy.signOut}
                      {:else}
                        {copy.deviceSignOut}
                      {/if}
                    </Button>
                  </div>
                {/if}
              </Card>
            </li>
          {/each}
          {#each unboundSessions as s (s.id)}
            {@const device = sessionDeviceLabel(s.userAgent, copy.deviceUnknown)}
            {@const lastSeen = sessionLastSeenLabel(s.lastSeenAt, copy.locale)}
            <li>
              <Card size="sm" class="flex-row flex-wrap items-center justify-between gap-3 px-4">
                <div class="flex flex-col gap-0.5">
                  <span class="text-sm font-medium">
                    {device}
                    {#if s.isCurrent}
                      <span
                        class="ms-2 rounded-pill bg-accent-soft px-2 py-0.5 text-xs text-accent"
                      >{copy.sessionThis}</span>
                    {/if}
                  </span>
                  {#if lastSeen}
                    <span class="text-xs text-fg-3">{copy.sessionLastSeen(lastSeen)}</span>
                  {/if}
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={terminatePendingId === s.id}
                  onclick={() => terminate(s.id, s.isCurrent)}
                >
                  {sessionActionLabel(s, terminatePendingId)}
                </Button>
              </Card>
            </li>
          {/each}
        </ul>
        {#if terminateError}
          <p role="alert" aria-live="assertive" class="text-sm text-destructive">{terminateError}</p>
        {/if}
      {/if}
    </section>

    <section class="flex flex-col gap-4">
      <h2 class="text-lg font-medium">{copy.securityHeading}</h2>
      <div class="flex flex-col gap-5">
        <TwoFactorControls />

        <Card class="gap-3 p-5">
          <div class="flex flex-col gap-1">
            <h3 class="text-base font-medium">{copy.passkeysHeading}</h3>
            <p class="text-sm text-fg-2">{copy.passkeysDescription}</p>
          </div>
          {#if !passkey.supported}
            <p class="text-sm text-fg-2">{copy.passkeysUnsupported}</p>
          {:else}
            {#if passkey.passkeys.length > 0}
              <ul class="flex flex-col gap-2">
                {#each passkey.passkeys as pk (pk.id)}
                  <li class="flex items-center justify-between gap-3">
                    <span class="text-sm">{pk.label || copy.passkeyDefaultLabel}</span>
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={passkey.pending}
                      onclick={() => passkey.remove(pk.id)}
                    >
                      {copy.passkeyRemove}
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
              {passkey.pending ? copy.passkeyAdding : copy.passkeyAdd}
            </Button>
          {/if}
        </Card>
      </div>
    </section>

    <FreshAuthControls />

    <ConnectedAccounts refreshKey={passkeyKey} onAnonymous={handleAnonymous} />

    <DeleteAccountControls />
  </div>
{/if}
