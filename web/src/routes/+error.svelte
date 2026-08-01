<script lang="ts">
  import { Container } from "$lib/components";
  import { SITE } from "$lib/config/site";

  let { status = 404, error }: { status?: number; error?: App.Error } = $props();
  const isNotFound = $derived(status === 404);
  const title = $derived(isNotFound ? "Page not found" : `${status} — error`);
</script>

<svelte:head>
  <title>{title} · {SITE.name}</title>
  <meta name="robots" content="noindex" />
  <meta name="description" content={`This page does not exist on ${SITE.domain}.`} />
</svelte:head>

<Container class="flex min-h-[60vh] flex-col items-center justify-center gap-4 py-24 text-center">
  <span class="font-mono text-xs uppercase tracking-wide text-fg-3">{status}</span>
  <h1 class="text-3xl font-medium tracking-[-0.02em]">
    {isNotFound ? "Page not found" : "Something went wrong"}
  </h1>
  <p class="max-w-md text-fg-2">
    {isNotFound
      ? `The page you're looking for isn't on ${SITE.domain}.`
      : (error?.message ?? "Unexpected error.")}
  </p>
  <a class="font-mono text-sm text-accent hover:underline" href="/">← Back to {SITE.name}</a>
</Container>
