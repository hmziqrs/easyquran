<script lang="ts">
  import type { AccountClient } from "#lib/auth/account-client.js";
  import ConnectedAccounts from "../ConnectedAccounts.svelte";

  interface Props {
    client: AccountClient;
    navigate?: (url: string) => void;
  }

  let { client, navigate }: Props = $props();

  let key = $state("0");
</script>

<ConnectedAccounts {client} {navigate} refreshKey={key} />
<button
  type="button"
  onclick={() => {
    key = `${key}-bump`;
  }}>bump</button
>
