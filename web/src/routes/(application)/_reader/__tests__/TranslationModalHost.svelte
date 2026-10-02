<script lang="ts">
  import { untrack } from "svelte";
  import TranslationModal from "../TranslationModal.svelte";

  // Same-instance open/close driver for the reopen (openedTick) test: the
  // modal component stays mounted while `open` toggles, exactly like the
  // TranslationButton binding in the reader header.
  let {
    primaryId,
    expose,
  }: {
    primaryId: string | null;
    expose: (setter: (open: boolean) => void) => void;
  } = $props();

  let open = $state(true);
  untrack(() =>
    expose((next: boolean) => {
      open = next;
    }),
  );
</script>

<TranslationModal bind:open {primaryId} />
