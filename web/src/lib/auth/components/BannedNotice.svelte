<script lang="ts">
  import type { BanNotice } from "#lib/auth/auth-copy.js";
  import { getAuthCopy } from "#lib/i18n/auth-copy.js";

  const copy = getAuthCopy();

  type Props = {
    notice: BanNotice;
    ondismiss: () => void;
  };

  let { notice, ondismiss }: Props = $props();
</script>

<section
  role="alert"
  aria-live="assertive"
  class="flex flex-col gap-3 rounded-md border border-danger/30 bg-danger/5 px-3.5 py-3 text-sm text-danger"
>
  <h2 class="text-base font-semibold">{copy.bannedHeading}</h2>
  <p>{copy.bannedBody}</p>
  {#if notice.reasonCode}
    <p class="text-xs">{copy.bannedReasonCode(notice.reasonCode)}</p>
  {/if}
  {#if notice.appealPath}
    <a href={notice.appealPath} class="text-primary underline underline-offset-2">
      {copy.bannedAppealLink}
    </a>
  {/if}
  <button type="button" class="self-start text-primary hover:underline" onclick={ondismiss}>
    {copy.useDifferentAccount}
  </button>
</section>
