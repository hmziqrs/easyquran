<script lang="ts">
  import { goto } from "$app/navigation";
  import { toast } from "svelte-sonner";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { accountClient } from "#lib/auth/account-client.js";
  import { purgeAccountLocalData } from "#lib/auth/account-purge.js";
  import { createLogoutFlow } from "#lib/auth/flows.svelte.js";
  import { isFreshAuthenticationRequired } from "#lib/auth/fresh-auth-client.js";
  import { getAccountCopy } from "#lib/i18n/account-copy.js";
  import { downloadExportFile } from "#lib/storage/data-export.js";

  const copy = getAccountCopy();
  const logout = createLogoutFlow();

  let typed = $state("");
  let pending = $state(false);
  let error = $state<string | null>(null);

  // The server ends every session before it erases. On success this removes the account's
  // browser data (outbox, device key, account-keyed storage); device-only reading data stays.
  async function remove(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (pending) return;
    const email = authState.user?.email ?? "";
    const userId = authState.user?.id ?? null;
    if (typed.trim().toLowerCase() !== email.trim().toLowerCase()) {
      error = copy.deleteMismatch;
      return;
    }
    pending = true;
    error = null;
    try {
      const res = await accountClient.deleteAccount(typed.trim());
      if (res.status === "ok") {
        typed = "";
        toast.success(copy.deleteDone);
        await logout.run();
        // After logout: the logout hook may still write account-keyed state; purge runs last.
        if (userId !== null) await purgeAccountLocalData(userId);
        await goto("/");
        return;
      }
      error = isFreshAuthenticationRequired(res.error) ? copy.deleteFreshAuth : copy.genericError;
    } catch {
      error = copy.genericError;
    } finally {
      pending = false;
    }
  }
</script>

<section class="flex flex-col gap-4 rounded-xl border border-line-2 bg-bg-2 p-5">
  <div class="flex flex-col gap-1">
    <h2 class="text-lg font-medium">{copy.deleteHeading}</h2>
    <p class="text-sm text-fg-2">{copy.deleteDescription}</p>
  </div>
  <div class="flex flex-col gap-2">
    <p class="text-sm text-fg-2">{copy.deleteExportHint}</p>
    <Button variant="secondary" class="self-start" onclick={() => downloadExportFile()}>
      {copy.deleteExport}
    </Button>
  </div>
  <form class="flex flex-col gap-3" onsubmit={remove}>
    <div class="flex flex-col gap-1.5">
      <Label for="delete-account-email">{copy.deleteFieldEmail}</Label>
      <Input
        id="delete-account-email"
        type="email"
        autocomplete="off"
        maxlength={320}
        required
        bind:value={typed}
        disabled={pending}
      />
    </div>
    <Button type="submit" variant="secondary" class="self-start text-destructive" disabled={pending}>
      {pending ? copy.deleteDeleting : copy.deleteSubmit}
    </Button>
  </form>
  {#if error}
    <p role="alert" aria-live="assertive" class="text-sm text-destructive">{error}</p>
  {/if}
</section>
