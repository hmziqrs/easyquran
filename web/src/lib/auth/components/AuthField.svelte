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
  <Label for={id} class="text-[13px] font-medium text-foreground-secondary">{label}</Label>
  <div class="relative">
    <Input
      class={cn("peer h-11 rounded-lg", leadingIcon && "ps-10", trailing && "pe-11", className)}
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
        class="pointer-events-none absolute inset-y-0 start-0 flex items-center ps-3.5 text-muted transition-colors peer-focus:text-primary"
      >
        {@render leadingIcon()}
      </span>
    {/if}
    {#if trailing}
      <div class="absolute inset-y-0 end-0 flex items-center pe-1.5">
        {@render trailing()}
      </div>
    {/if}
  </div>
  {#if error}
    <span id={errorId} role="alert" class="text-xs text-destructive">{error}</span>
  {:else if hint}
    <span id={hintId} class="text-xs text-muted">{hint}</span>
  {/if}
</div>
