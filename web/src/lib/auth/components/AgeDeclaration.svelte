<script lang="ts">
  import { Button } from "#lib/components/ui/button/index.js";
  import { Card } from "#lib/components/ui/card/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { accountClient } from "#lib/auth/account-client.js";
  import { authState } from "#lib/auth/auth-state.svelte.js";
  import { forgetDeviceIdFor } from "#lib/device/identity.js";
  import { getAccountCopy } from "#lib/i18n/account-copy.js";

  /**
   * One-time age record for accounts created without a date of birth (A34). Account page
   * only, never a gate: the reader and sync work the same whether or not this is answered.
   * The date is sent once and kept only in this component's state, then cleared.
   */
  const copy = getAccountCopy();

  let dateOfBirth = $state("");
  let pending = $state(false);
  let error = $state<string | null>(null);
  let alreadyDeclared = $state(false);
  let done = $state(false);

  const showPrompt = $derived(
    authState.user !== null &&
      authState.user.age_declared !== true &&
      !alreadyDeclared &&
      !done,
  );

  async function submit(): Promise<void> {
    if (pending) return;
    if (!/^\d{4}-\d{2}-\d{2}$/u.test(dateOfBirth)) {
      error = copy.ageInvalid;
      return;
    }
    pending = true;
    error = null;
    const res = await accountClient.declareAge(dateOfBirth);
    pending = false;
    dateOfBirth = "";
    if (res.status === "ok" && res.data) {
      const profile = res.data;
      authState.setUser(profile);
      done = true;
      // A minor's device identity is memory-only from here on. Reload so every sync
      // store starts with that id; nothing persisted under the old id survives.
      if (profile.is_minor === true) {
        forgetDeviceIdFor(String(profile.id));
        location.reload();
      }
      return;
    }
    if (res.httpStatus === 409) {
      alreadyDeclared = true;
      return;
    }
    error = res.httpStatus === 400 ? copy.ageInvalid : copy.genericError;
  }
</script>

{#if showPrompt || done}
  <section class="flex flex-col gap-4" aria-labelledby="account-age-heading">
    <h2 id="account-age-heading" class="text-lg font-medium">{copy.ageHeading}</h2>
    {#if done}
      <p role="status" aria-live="polite" class="text-sm text-accent">{copy.ageDone}</p>
    {:else}
      <Card class="gap-3 p-5">
        <p class="text-sm text-fg-2">{copy.ageDescription}</p>
        <form
          class="flex flex-col gap-3"
          onsubmit={(event: SubmitEvent) => {
            event.preventDefault();
            void submit();
          }}
        >
          <div class="flex flex-col gap-1.5">
            <Label for="account-age-dob">{copy.ageField}</Label>
            <Input
              id="account-age-dob"
              type="date"
              autocomplete="bday"
              bind:value={dateOfBirth}
              disabled={pending}
              required
            />
          </div>
          {#if error}
            <p role="alert" aria-live="assertive" class="text-sm text-destructive">{error}</p>
          {/if}
          <Button variant="accent" class="self-start" type="submit" disabled={pending}>
            {pending ? copy.ageSaving : copy.ageSubmit}
          </Button>
        </form>
      </Card>
    {/if}
  </section>
{:else if alreadyDeclared}
  <p role="status" aria-live="polite" class="text-sm text-fg-2">{copy.ageAlready}</p>
{/if}
