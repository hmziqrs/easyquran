<script lang="ts">
  import LockIcon from "phosphor-svelte/lib/LockIcon";
  import EyeIcon from "phosphor-svelte/lib/EyeIcon";
  import EyeSlashIcon from "phosphor-svelte/lib/EyeSlashIcon";
  import AuthField from "./AuthField.svelte";

  type Props = {
    id: string;
    label: string;
    value?: string;
    autocomplete?: string;
    placeholder?: string;
    error?: string | null;
    minlength?: number;
  };

  let {
    id,
    label,
    value = $bindable(""),
    autocomplete = "current-password",
    placeholder,
    error = null,
    minlength,
  }: Props = $props();

  let visible = $state(false);
  let type = $derived(visible ? "text" : "password");
</script>

<AuthField
  {id}
  {label}
  {type}
  {autocomplete}
  {placeholder}
  {minlength}
  bind:value
  {error}
>
  {#snippet leadingIcon()}
    <LockIcon weight="fill" size={16} aria-hidden="true" />
  {/snippet}
  {#snippet trailing()}
    <button
      type="button"
      aria-label={visible ? "Hide password" : "Show password"}
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
