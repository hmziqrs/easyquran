<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLInputAttributes } from "svelte/elements";
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { cn } from "$lib/utils";

  type Props = {
    id: string;
    name?: string;
    label: string;
    value: string;
    type?: string;
    autocomplete?: string;
    placeholder?: string;
    inputmode?: string;
    maxlength?: number;
    minlength?: number;
    hint?: string;
    leadingIcon?: Snippet;
    trailing?: Snippet;
    error?: string | null;
    class?: string;
    oninput?: (value: string) => void;
    onblur?: () => void;
  };

  let {
    id,
    name,
    label,
    value,
    type = "text",
    autocomplete,
    placeholder,
    inputmode,
    maxlength,
    minlength,
    hint,
    leadingIcon,
    trailing,
    error = null,
    class: className,
    oninput,
    onblur,
  }: Props = $props();

  const hintId = $derived(hint ? `${id}-hint` : undefined);
  const errorId = $derived(error ? `${id}-error` : undefined);
  const describedBy = $derived([errorId, hintId].filter(Boolean).join(" ") || undefined);
</script>

<div class="flex flex-col gap-1.5">
  <Label for={id} class="text-[13px] font-medium text-fg-2">{label}</Label>
  <div class="relative">
    <Input
      class={cn("peer h-11 rounded-lg", leadingIcon && "pl-10", trailing && "pr-11", className)}
      {id}
      name={name ?? id}
      {type}
      autocomplete={autocomplete as HTMLInputAttributes["autocomplete"]}
      {placeholder}
      inputmode={inputmode as HTMLInputAttributes["inputmode"]}
      {maxlength}
      {minlength}
      {value}
      oninput={(event) => oninput?.(event.currentTarget.value)}
      onblur={() => onblur?.()}
      aria-invalid={Boolean(error)}
      aria-describedby={describedBy}
      data-invalid={Boolean(error)}
    />
    {#if leadingIcon}
      <span
        class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3.5 text-fg-4 transition-colors peer-focus:text-accent"
      >
        {@render leadingIcon()}
      </span>
    {/if}
    {#if trailing}
      <div class="absolute inset-y-0 right-0 flex items-center pr-1.5">
        {@render trailing()}
      </div>
    {/if}
  </div>
  {#if error}
    <span id={errorId} role="alert" class="text-xs text-destructive">{error}</span>
  {:else if hint}
    <span id={hintId} class="text-xs text-fg-3">{hint}</span>
  {/if}
</div>
