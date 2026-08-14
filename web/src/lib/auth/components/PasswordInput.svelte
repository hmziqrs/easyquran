<script lang="ts">
  import LockIcon from "phosphor-svelte/lib/LockIcon";
  import EyeIcon from "phosphor-svelte/lib/EyeIcon";
  import EyeSlashIcon from "phosphor-svelte/lib/EyeSlashIcon";
  import AuthField from "./AuthField.svelte";
  import { getAuthCopy } from "$lib/i18n/auth-copy";

  const copy = getAuthCopy();

  type Props = {
    id: string;
    name?: string;
    label: string;
    value: string;
    autocomplete?: string;
    placeholder?: string;
    hint?: string;
    error?: string | null;
    minlength?: number;
    oninput?: (value: string) => void;
    onblur?: () => void;
  };

  let {
    id,
    name,
    label,
    value,
    autocomplete = "current-password",
    placeholder,
    hint,
    error = null,
    minlength,
    oninput,
    onblur,
  }: Props = $props();

  let visible = $state(false);
  let type = $derived(visible ? "text" : "password");
</script>

<AuthField
  {id}
  {name}
  {label}
  {type}
  {autocomplete}
  {placeholder}
  {minlength}
  {hint}
  {value}
  {error}
  {oninput}
  {onblur}
>
  {#snippet leadingIcon()}
    <LockIcon weight="fill" size={16} aria-hidden="true" />
  {/snippet}
  {#snippet trailing()}
    <button
      type="button"
      aria-label={visible ? copy.hidePasswordAria : copy.showPasswordAria}
      class="inline-flex h-8 w-8 items-center justify-center rounded-md text-fg-3 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      onclick={() => (visible = !visible)}
    >
      {#if visible}
        <EyeIcon weight="fill" size={16} aria-hidden="true" />
      {:else}
        <EyeSlashIcon weight="fill" size={16} aria-hidden="true" />
      {/if}
    </button>
  {/snippet}
</AuthField>
