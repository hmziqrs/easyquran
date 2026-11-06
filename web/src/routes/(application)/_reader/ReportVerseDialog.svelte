<script lang="ts">
  import { authClient } from "#lib/auth/auth-client.js";
  import { Button } from "#lib/components/ui/button/index.js";
  import * as Dialog from "#lib/components/ui/dialog/index.js";
  import { Input } from "#lib/components/ui/input/index.js";
  import { Textarea } from "#lib/components/ui/textarea/index.js";
  import { appLocale } from "#lib/i18n/app-locale.js";
  import {
    reader_report_cancel,
    reader_report_contact,
    reader_report_description,
    reader_report_details,
    reader_report_failed,
    reader_report_send,
    reader_report_sending,
    reader_report_sent,
    reader_report_title,
    reader_report_too_long,
  } from "#lib/i18n/m/reader.js";
  import { buildReportPayload, REPORT_DETAILS_MAX } from "#lib/report/payload.js";

  type Status = "idle" | "invalid" | "too_long" | "sending" | "sent" | "error";

  let {
    open = $bindable(false),
    sourceId,
    verseKey,
  }: {
    open?: boolean;
    /** Immutable translation database id the verse is read in (e.g. `ar.jalalayn`). */
    sourceId: string;
    /** `surah:ayah` of the reported verse. */
    verseKey: string;
  } = $props();

  const msg = { locale: appLocale() };
  let details = $state("");
  let contactEmail = $state("");
  let status = $state<Status>("idle");

  const statusMessage = $derived.by(() => {
    if (status === "sent") return reader_report_sent(undefined, msg);
    if (status === "error" || status === "invalid") return reader_report_failed(undefined, msg);
    if (status === "too_long") return reader_report_too_long({ max: REPORT_DETAILS_MAX }, msg);
    return "";
  });

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (status === "sending" || status === "sent") return;
    const built = buildReportPayload({
      targetKind: "translation",
      targetId: `${sourceId}:${verseKey}`,
      reason: "other",
      details,
      contactEmail,
    });
    if (!built.ok) {
      status = built.error === "details_too_long" ? "too_long" : "invalid";
      return;
    }
    status = "sending";
    try {
      const res = await authClient.unsafeRequest("/report/v1/submit", {
        method: "POST",
        body: built.body,
      });
      status = res.ok ? "sent" : "error";
    } catch {
      status = "error";
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content>
    <div class="flex flex-col gap-1.5">
      <Dialog.Title>{reader_report_title(undefined, msg)}</Dialog.Title>
      <Dialog.Description>{reader_report_description(undefined, msg)}</Dialog.Description>
    </div>

    <form class="flex flex-col gap-4" onsubmit={submit}>
      <label class="flex flex-col gap-1.5 text-caption text-foreground-secondary">
        {reader_report_details(undefined, msg)}
        <Textarea bind:value={details} rows={4} class="resize-y" />
      </label>

      <label class="flex flex-col gap-1.5 text-caption text-foreground-secondary">
        {reader_report_contact(undefined, msg)}
        <Input type="email" autocomplete="email" maxlength={320} bind:value={contactEmail} />
      </label>

      {#if statusMessage !== ""}
        <p role="status" class="text-caption text-muted-foreground">{statusMessage}</p>
      {/if}

      <div class="flex flex-wrap justify-end gap-2">
        <Button type="button" variant="ghost" size="sm" onclick={() => (open = false)}>
          {reader_report_cancel(undefined, msg)}
        </Button>
        <Button type="submit" size="sm" disabled={status === "sending" || status === "sent"}>
          {status === "sending" ? reader_report_sending(undefined, msg) : reader_report_send(undefined, msg)}
        </Button>
      </div>
    </form>
  </Dialog.Content>
</Dialog.Root>
