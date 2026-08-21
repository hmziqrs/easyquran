<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Card } from "$lib/components";
  import { authModal } from "$lib/auth/auth-modal.svelte";
  import { authState } from "$lib/auth/auth-state.svelte";
  import { publicHref } from "$lib/i18n/public-href";
  import type { SettingsCopy } from "$lib/i18n/settings-copy";

  let {
    id,
    heading,
    copy,
  }: {
    id: string;
    heading: string;
    copy: SettingsCopy["account"];
  } = $props();

  const signedIn = $derived(authState.status === "authenticated");
  const user = $derived(authState.user);
  const accountHref = $derived(publicHref("/account"));
</script>

<Card id={id} class="scroll-mt-24">
  <h2 class="text-sm font-semibold text-fg">{heading}</h2>
  <p class="mt-1 text-xs text-fg-3">{copy.intro}</p>

  {#if signedIn}
    <div class="mt-3 flex flex-col gap-3">
      <div class="flex flex-col gap-0.5">
        <h3 class="text-xs font-medium text-fg">{copy.signedInHeading}</h3>
        {#if user?.name}
          <p class="text-xs text-fg-2">{copy.signedInAs(user.name)}</p>
        {/if}
        {#if user?.email}
          <p class="text-xs text-fg-3">{user.email}</p>
        {/if}
      </div>
      <Button variant="ghost" size="sm" href={accountHref} class="self-start">
        {copy.open}
      </Button>
    </div>
  {:else}
    <div class="mt-3 flex flex-col gap-3">
      <p class="text-xs leading-relaxed text-fg-2">{copy.signedOutNote}</p>
      <Button
        variant="accent"
        size="sm"
        class="self-start"
        onclick={() => authModal.show("login")}>{copy.signIn}</Button
      >
    </div>
  {/if}

  <p class="mt-3 text-[11px] leading-snug text-fg-4">{copy.deviceNote}</p>
</Card>
