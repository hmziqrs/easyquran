<script lang="ts">
  import { goto } from "$app/navigation";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { accountClient, identityProviderLabel, isIdentityProvider, type IdentityProvider } from "#lib/auth/account-client.js";
  import { createLogoutFlow } from "#lib/auth/flows.svelte.js";
  import { freshAuthClient, type FreshAuthStatus } from "#lib/auth/fresh-auth-client.js";

  let status = $state<FreshAuthStatus | null>(null);
  let providers = $state<IdentityProvider[]>([]);
  let password = $state("");
  let code = $state("");
  let pending = $state(false);
  let error = $state<string | null>(null);
  let sequence = 0;
  const logout = createLogoutFlow();

  $effect(() => {
    const owner = authState.user?.id;
    sequence += 1;
    status = null;
    providers = [];
    password = "";
    code = "";
    if (owner !== undefined) void refresh();
  });

  async function refresh(): Promise<void> {
    const current = ++sequence;
    error = null;
    try {
      const [next, identities] = await Promise.all([
        freshAuthClient.status(),
        accountClient.getIdentities(),
      ]);
      if (current !== sequence) return;
      status = next;
      const linked = identities.data?.identities.map((identity) => identity.provider) ?? [];
      const legacyProvider = authState.user?.oauth_provider;
      if (linked.length === 0 && legacyProvider && isIdentityProvider(legacyProvider)) {
        providers = [legacyProvider];
      } else {
        providers = linked;
      }
      if (!next) error = "Couldn't check confirmation. Try again.";
    } catch {
      if (current === sequence) error = "Couldn't check confirmation. Try again.";
    }
  }

  async function confirm(): Promise<void> {
    if (pending) return;
    const owner = authState.user?.id;
    pending = true;
    error = null;
    try {
      const result = await freshAuthClient.confirm(password, code.trim());
      if (owner !== authState.user?.id) return;
      password = "";
      code = "";
      if (result) {
        error = result;
        return;
      }
      await refresh();
    } catch {
      if (owner === authState.user?.id) error = "Confirmation failed. Try again.";
    } finally {
      pending = false;
    }
  }

  async function providerSignIn(provider: IdentityProvider): Promise<void> {
    const { createOAuthFlow } = await import("#lib/auth/oauth-flow.svelte.js");
    const flow = createOAuthFlow(provider);
    if (!(await flow.begin("/account"))) error = "Sign-in couldn't start. Try again.";
  }

  async function signInAgain(): Promise<void> {
    if (await logout.run()) await goto("/login");
  }
</script>

<section class="flex flex-col gap-4 rounded-xl border border-line-2 bg-bg-2 p-5">
  <div class="flex flex-col gap-1">
    <h2 class="text-lg font-medium">Security confirmation</h2>
    <p class="text-sm text-fg-2">Confirm your identity before changing sign-in methods or security settings. Each confirmation permits one change within five minutes.</p>
  </div>
  {#if status?.ready}
    <p role="status" class="text-sm text-accent">Ready for one security change.</p>
  {/if}
  {#if status?.has_password}
    <form class="flex flex-col gap-3" onsubmit={(event) => { event.preventDefault(); void confirm(); }}>
      <div class="flex flex-col gap-1.5">
        <Label for="fresh-auth-password">Current password</Label>
        <Input id="fresh-auth-password" type="password" autocomplete="current-password" minlength={12} maxlength={256} required bind:value={password} disabled={pending} />
      </div>
      {#if status.two_fa_enabled}
        <div class="flex flex-col gap-1.5">
          <Label for="fresh-auth-code">Authenticator or backup code</Label>
          <Input id="fresh-auth-code" type="text" autocomplete="one-time-code" maxlength={64} required bind:value={code} disabled={pending} />
        </div>
      {/if}
      <Button type="submit" variant="accent" class="self-start" disabled={pending}>{pending ? "Confirming…" : "Confirm identity"}</Button>
    </form>
  {/if}
  {#if providers.length > 0}
    <div class="flex flex-wrap gap-2">
      {#each providers as provider (provider)}
        <Button variant="secondary" size="sm" disabled={pending} onclick={() => providerSignIn(provider)}>Confirm with {identityProviderLabel(provider)}</Button>
      {/each}
    </div>
    <p class="text-xs text-fg-3">Use your linked provider account. If you enabled two-factor authentication, finish that step too.</p>
  {:else if status && !status.has_password}
    <p class="text-sm text-fg-2">Sign out and sign in again, then return to Account to make your security change.</p>
    <Button variant="secondary" class="self-start" disabled={logout.pending} onclick={signInAgain}>Sign in again</Button>
  {/if}
  {#if error}
    <p role="alert" class="text-sm text-destructive">{error}</p>
  {/if}
  <Button variant="ghost" size="sm" class="self-start" disabled={pending} onclick={refresh}>Check confirmation</Button>
</section>
