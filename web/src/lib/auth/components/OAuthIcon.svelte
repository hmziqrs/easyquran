<script lang="ts">
  import { siGoogle, siApple, siFacebook, siGithub, type SimpleIcon } from "simple-icons";
  import type { OAuthProvider } from "$lib/auth/oauth-flow.svelte";

  type Props = {
    provider: OAuthProvider;
    size?: number;
    class?: string;
  };

  let { provider, size = 16, class: className }: Props = $props();

  const ICONS = {
    google: siGoogle,
    apple: siApple,
    facebook: siFacebook,
    github: siGithub,
  } satisfies Record<OAuthProvider, SimpleIcon>;

  let icon = $derived(ICONS[provider]);
</script>

<svg
  role="img"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  class={className}
  aria-hidden="true"
  focusable="false"
>
  <path d={icon.path} fill="currentColor" />
</svg>
