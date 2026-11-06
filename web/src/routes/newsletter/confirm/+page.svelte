<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { authClient } from "#lib/auth/auth-client.js";
  import { Container } from "#lib/components/index.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import {
    newsletter_confirm_error,
    newsletter_confirm_heading,
    newsletter_confirm_missing,
    newsletter_confirm_pending,
    newsletter_confirm_seo_description,
    newsletter_confirm_seo_title,
    newsletter_confirm_success,
  } from "#lib/i18n/m/newsletter.js";

  type Status = "pending" | "sent" | "missing" | "error";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));
  let status = $state<Status>("pending");

  // The confirm link carries email and token in the fragment, which browsers never
  // send to a server. Read them once, then drop them from the address bar.
  onMount(async () => {
    const params = new URLSearchParams(window.location.hash.slice(1));
    const email = params.get("email")?.trim() ?? "";
    const token = params.get("token")?.trim() ?? "";
    history.replaceState(null, "", window.location.pathname);
    if (!email || !token) {
      status = "missing";
      return;
    }
    try {
      const res = await authClient.unsafeRequest("/newsletter/v1/confirm", {
        method: "POST",
        body: { email, token },
      });
      status = res.ok ? "sent" : "error";
    } catch {
      status = "error";
    }
  });

  const message = $derived.by(() => {
    if (status === "sent") return newsletter_confirm_success(undefined, { locale });
    if (status === "missing") return newsletter_confirm_missing(undefined, { locale });
    if (status === "error") return newsletter_confirm_error(undefined, { locale });
    return newsletter_confirm_pending(undefined, { locale });
  });
</script>

<svelte:head>
  <title>{newsletter_confirm_seo_title(undefined, { locale })}</title>
  <meta name="description" content={newsletter_confirm_seo_description(undefined, { locale })} />
  <meta name="robots" content="noindex" />
</svelte:head>

<Container class="py-12 md:py-16">
  <div class="flex max-w-[680px] flex-col gap-4">
    <h1 class="text-h1">{newsletter_confirm_heading(undefined, { locale })}</h1>
    <p role="status" class="text-body text-foreground">{message}</p>
  </div>
</Container>
