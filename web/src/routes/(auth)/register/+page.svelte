<script lang="ts">
  import { goto } from "$app/navigation";
  import AuthForm from "$lib/auth/components/AuthForm.svelte";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { createRegisterFlow } from "$lib/auth/flows.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";

  const flow = createRegisterFlow();

  async function handleSubmit(): Promise<void> {
    if (flow.step === "totp") {
      const ok = await flow.login.submitTotp();
      if (!ok) return;
      const user = authState.user;
      await goto(user && !user.is_verified ? "/verify-email" : "/app");
      return;
    }
    const ok = await flow.submit();
    if (!ok) return;
    if (flow.step === "unverified") await goto("/verify-email");
    else if (flow.step === "done") await goto("/app");
  }

  function invalid(field: string): boolean {
    return Boolean(flow.fieldErrors[field]);
  }

  const totpServerError = $derived(
    flow.login.genericError ?? (flow.login.fieldErrors.code ? null : flow.genericError),
  );
  const totpInvalid = $derived(Boolean(flow.login.fieldErrors.code));
</script>

{#if flow.step === "totp"}
  <AuthForm
    heading="Two-factor code"
    subheading="Enter the 6-digit code from your authenticator app to finish signing in."
    submitLabel="Continue"
    pending={flow.login.pending}
    serverError={totpServerError}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="register-totp">Authentication code</Label>
      <Input
        id="register-totp"
        type="text"
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength={6}
        bind:value={flow.login.code}
        aria-invalid={totpInvalid}
        data-invalid={totpInvalid}
      />
      {#if flow.login.fieldErrors.code}
        <span class="text-xs text-destructive">{flow.login.fieldErrors.code}</span>
      {/if}
    </div>
  </AuthForm>
{:else}
  <AuthForm
    heading="Create your account"
    subheading="A free account syncs your bookmarks, notes and reading place across devices."
    submitLabel="Create account"
    pending={flow.pending}
    serverError={flow.genericError}
    onsubmit={handleSubmit}
  >
    <div class="flex flex-col gap-1.5">
      <Label for="register-name">Display name</Label>
      <Input
        id="register-name"
        type="text"
        autocomplete="name"
        bind:value={flow.name}
        aria-invalid={invalid("name")}
        data-invalid={invalid("name")}
      />
      {#if flow.fieldErrors.name}
        <span class="text-xs text-destructive">{flow.fieldErrors.name}</span>
      {/if}
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="register-email">Email</Label>
      <Input
        id="register-email"
        type="email"
        autocomplete="email"
        bind:value={flow.email}
        aria-invalid={invalid("email")}
        data-invalid={invalid("email")}
      />
      {#if flow.fieldErrors.email}
        <span class="text-xs text-destructive">{flow.fieldErrors.email}</span>
      {/if}
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="register-password">Password</Label>
      <Input
        id="register-password"
        type="password"
        autocomplete="new-password"
        minlength={12}
        bind:value={flow.password}
        aria-invalid={invalid("password")}
        data-invalid={invalid("password")}
      />
      {#if flow.fieldErrors.password}
        <span class="text-xs text-destructive">{flow.fieldErrors.password}</span>
      {/if}
    </div>
    <div class="flex flex-col gap-1.5">
      <Label for="register-confirm">Confirm password</Label>
      <Input
        id="register-confirm"
        type="password"
        autocomplete="new-password"
        minlength={12}
        bind:value={flow.confirmPassword}
        aria-invalid={invalid("confirm_password")}
        data-invalid={invalid("confirm_password")}
      />
      {#if flow.fieldErrors.confirm_password}
        <span class="text-xs text-destructive">{flow.fieldErrors.confirm_password}</span>
      {/if}
    </div>
    {#snippet footer()}
      <span>Already have an account? <a href="/login" class="text-accent hover:underline">Sign in</a></span>
    {/snippet}
  </AuthForm>
{/if}
