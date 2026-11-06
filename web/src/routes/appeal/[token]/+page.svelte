<script lang="ts">
  import { page } from "$app/state";
  import { APPEAL_PATH, APPEAL_TOKEN_PATTERN } from "#lib/appeal/route.js";
  import { authClient } from "#lib/auth/auth-client.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import { Label } from "#lib/components/ui/label/index.js";
  import { Textarea } from "#lib/components/ui/textarea/index.js";
  import {
    appeal_error,
    appeal_invalid_link,
    appeal_page_title,
    appeal_received,
    appeal_sending,
    appeal_statement_help,
    appeal_statement_invalid,
    appeal_statement_label,
    appeal_submit,
  } from "#lib/i18n/m/appeal.js";

  // Mirrors the API cap (appeal_v1 MAX_STATEMENT_CHARS), counted in code points.
  const MAX_STATEMENT_CHARS = 2_000;

  type Phase = "idle" | "sending" | "received" | "error";

  const token = $derived(page.params.token ?? "");
  const tokenLooksValid = $derived(APPEAL_TOKEN_PATTERN.test(token));

  let statement = $state("");
  let statementInvalid = $state(false);
  let phase = $state<Phase>("idle");

  async function send(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const text = statement.trim();
    const length = [...text].length;
    if (length === 0 || length > MAX_STATEMENT_CHARS) {
      statementInvalid = true;
      return;
    }
    statementInvalid = false;
    phase = "sending";
    try {
      // The token travels in the JSON body, never in a URL.
      const res = await authClient.unsafeRequest<{ received: boolean }>(
        `${APPEAL_PATH}/v1/submit`,
        { method: "POST", body: { token, statement: text } },
      );
      if (res.ok) {
        phase = "received";
        statement = "";
        return;
      }
      if (res.status === 400) {
        statementInvalid = true;
        phase = "idle";
        return;
      }
      phase = "error";
    } catch {
      phase = "error";
    }
  }
</script>

<svelte:head>
  <title>{appeal_page_title()}</title>
  <meta name="robots" content="noindex" />
</svelte:head>

<main class="mx-auto flex w-full max-w-xl flex-col gap-6 px-4 py-10">
  <h1 class="text-2xl font-semibold text-foreground">{appeal_page_title()}</h1>

  {#if !tokenLooksValid}
    <p role="alert" class="text-sm text-danger">{appeal_invalid_link()}</p>
  {:else if phase === "received"}
    <p role="status" class="text-sm text-foreground">{appeal_received()}</p>
  {:else}
    <form class="flex flex-col gap-3" onsubmit={send}>
      <Label for="appeal-statement">{appeal_statement_label()}</Label>
      <Textarea
        id="appeal-statement"
        name="appeal-statement"
        rows={8}
        aria-describedby="appeal-statement-help"
        aria-invalid={statementInvalid ? "true" : undefined}
        bind:value={statement}
      />
      <p id="appeal-statement-help" class="text-xs text-muted-foreground">
        {appeal_statement_help()}
      </p>
      {#if statementInvalid}
        <p role="alert" class="text-sm text-danger">{appeal_statement_invalid()}</p>
      {/if}
      {#if phase === "error"}
        <p role="alert" class="text-sm text-danger">{appeal_error()}</p>
      {/if}
      <Button type="submit" class="self-start" disabled={phase === "sending"}>
        {phase === "sending" ? appeal_sending() : appeal_submit()}
      </Button>
    </form>
  {/if}
</main>
