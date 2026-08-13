<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { focusFirstInvalid } from "$lib/auth/components/auth-form-focus";
  import { cn } from "$lib/utils";
  import type { Snippet } from "svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

  type Props = {
    heading: string;
    subheading?: string;
    submitLabel: string;
    pending?: boolean;
    serverError?: string | null;
    successNotice?: string | null;
    variant?: "page" | "modal";
    onsubmit: () => void | Promise<void>;
    children?: Snippet;
    footer?: Snippet;
  };

  let {
    heading,
    subheading,
    submitLabel,
    pending = false,
    serverError = null,
    successNotice = null,
    variant = "page",
    onsubmit,
    children,
    footer,
  }: Props = $props();

  let headingClass = $derived(
    variant === "modal"
      ? "text-[22px] font-semibold leading-tight tracking-[-0.01em]"
      : "text-[28px] font-semibold leading-tight tracking-[-0.02em]",
  );
  let subheadingClass = $derived(variant === "modal" ? "text-[14px] text-fg-2" : "text-[15px] text-fg-2");

  let formEl: HTMLFormElement | null = $state(null);

  async function handleSubmit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (pending) return;
    await onsubmit();
    if (!formEl) return;
    focusFirstInvalid(formEl);
  }
</script>

<form bind:this={formEl} onsubmit={handleSubmit} class="flex flex-col gap-5" novalidate>
  <div class="flex flex-col gap-2">
    <h1 class={headingClass}>{heading}</h1>
    {#if subheading}
      <p class={cn("leading-relaxed", subheadingClass)}>{subheading}</p>
    {/if}
  </div>

  {#if serverError}
    <p role="alert" aria-live="assertive" class="rounded-lg border border-destructive/30 bg-destructive/5 px-3.5 py-2.5 text-sm text-destructive">
      {serverError}
    </p>
  {:else if successNotice}
    <p role="status" aria-live="polite" class="rounded-lg border border-accent/30 bg-accent-soft px-3.5 py-2.5 text-sm text-accent">
      {successNotice}
    </p>
  {/if}

  <div class="flex flex-col gap-4">
    {#if children}{@render children()}{/if}
  </div>

  <Button type="submit" variant="accent" size="lg" disabled={pending} class="mt-1 w-full">
    {pending ? copy.pleaseWait : submitLabel}
  </Button>

  {#if footer}
    <div class="flex flex-col gap-1.5 text-center text-sm text-fg-2">
      {@render footer()}
    </div>
  {/if}
</form>
