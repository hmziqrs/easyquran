<script lang="ts">
  import { Button } from "$lib/components/ui/button";
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

<div id={id} tabindex="-1" class="scroll-mt-24">
  <h2 class="text-[17px] font-semibold tracking-[-0.02em] text-foreground">{heading}</h2>
  <p class="mt-1 max-w-[70ch] text-[14.5px] leading-relaxed text-foreground-secondary">{copy.intro}</p>

  <div class="mt-4 divide-y divide-border overflow-hidden rounded-xl border border-border-strong bg-surface">
    {#if signedIn}
      <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <div class="min-w-0">
          <span class="text-[14.5px] font-medium text-foreground">{copy.signedInHeading}</span>
          {#if user?.name}
            <p class="mt-0.5 text-caption text-muted">{copy.signedInAs(user.name)}</p>
          {/if}
          {#if user?.email}
            <p class="text-caption text-muted">{user.email}</p>
          {/if}
        </div>
        <Button variant="ghost" size="sm" href={accountHref} class="shrink-0">
          {copy.open}
        </Button>
      </div>
    {:else}
      <div class="flex flex-wrap items-center justify-between gap-4 px-4 py-3.5 sm:px-5">
        <p class="max-w-[50ch] min-w-0 text-caption leading-relaxed text-muted">
          {copy.signedOutNote}
        </p>
        <Button
          variant="accent"
          size="sm"
          class="shrink-0"
          onclick={() => authModal.show("login")}>{copy.signIn}</Button
        >
      </div>
    {/if}
  </div>

  <p class="mt-4 text-caption leading-snug text-muted">{copy.deviceNote}</p>
</div>
