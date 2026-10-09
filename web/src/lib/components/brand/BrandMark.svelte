<script lang="ts">
  import openPages from "#lib/assets/open-pages.svg";

  let { class: className = "size-6" }: { class?: string } = $props();
</script>

<span
  class={["inline-block bg-current", className]}
  style:mask={`url("${openPages}") center / contain no-repeat`}
  aria-hidden="true"
></span>
