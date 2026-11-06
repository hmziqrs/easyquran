<script lang="ts">
  import { page } from "$app/state";
  import { authClient } from "#lib/auth/auth-client.js";
  import { Container } from "#lib/components/index.js";
  import { marketingLocaleFromPath } from "#lib/i18n/marketing-copy.js";
  import {
    report_contact_label,
    report_details_label,
    report_error,
    report_heading,
    report_intro,
    report_invalid_target,
    report_reason_copyright,
    report_reason_csam,
    report_reason_harassment,
    report_reason_hate,
    report_reason_other,
    report_reason_sexual,
    report_reason_spam,
    report_reason_threat,
    report_reason_violence,
    report_reason_label,
    report_seo_description,
    report_seo_title,
    report_submit,
    report_success,
    report_target_comment,
    report_target_id_help,
    report_target_id_label,
    report_target_kind_label,
    report_target_post,
    report_target_translation,
  } from "#lib/i18n/m/report.js";
  import {
    buildReportPayload,
    REPORT_DETAILS_MAX,
    REPORT_FORM_TARGET_KINDS,
    REPORT_REASONS,
    type ReportReason,
    type ReportTargetKind,
  } from "#lib/report/payload.js";

  type Status = "idle" | "invalid" | "sending" | "sent" | "error";

  const locale = $derived(marketingLocaleFromPath(page.url.pathname));

  const REASON_LABEL = {
    spam: () => report_reason_spam(undefined, { locale }),
    harassment: () => report_reason_harassment(undefined, { locale }),
    hate: () => report_reason_hate(undefined, { locale }),
    sexual: () => report_reason_sexual(undefined, { locale }),
    violence: () => report_reason_violence(undefined, { locale }),
    copyright: () => report_reason_copyright(undefined, { locale }),
    csam: () => report_reason_csam(undefined, { locale }),
    threat: () => report_reason_threat(undefined, { locale }),
    other: () => report_reason_other(undefined, { locale }),
  } satisfies Record<ReportReason, () => string>;

  const KIND_LABEL = {
    post: () => report_target_post(undefined, { locale }),
    comment: () => report_target_comment(undefined, { locale }),
    translation: () => report_target_translation(undefined, { locale }),
  } satisfies Record<ReportTargetKind, () => string>;

  let targetKind = $state<ReportTargetKind>("post");
  let targetId = $state("");
  let reason = $state<ReportReason>("spam");
  let details = $state("");
  let contactEmail = $state("");
  let status = $state<Status>("idle");

  const inputClass =
    "w-full rounded-md border border-border bg-background px-3 py-2 text-body text-foreground focus-visible:outline-2 focus-visible:outline-focus-ring";

  const statusMessage = $derived.by(() => {
    if (status === "sent") return report_success(undefined, { locale });
    if (status === "error") return report_error(undefined, { locale });
    if (status === "invalid") return report_invalid_target(undefined, { locale });
    return "";
  });

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (status === "sending") return;
    const built = buildReportPayload({ targetKind, targetId, reason, details, contactEmail });
    if (!built.ok) {
      status = "invalid";
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

<svelte:head>
  <title>{report_seo_title(undefined, { locale })}</title>
  <meta name="description" content={report_seo_description(undefined, { locale })} />
  <meta name="robots" content="noindex" />
</svelte:head>

<Container class="py-12 md:py-16">
  <div class="flex max-w-[680px] flex-col gap-8">
    <div class="flex flex-col gap-3">
      <h1 class="text-h1">{report_heading(undefined, { locale })}</h1>
      <p class="text-body-l text-foreground-secondary">{report_intro(undefined, { locale })}</p>
    </div>

    <form class="flex flex-col gap-5" onsubmit={submit}>
      <div class="grid gap-5 sm:grid-cols-2">
        <label class="flex flex-col gap-2 text-body font-medium text-foreground">
          {report_target_kind_label(undefined, { locale })}
          <select class={inputClass} bind:value={targetKind}>
            {#each REPORT_FORM_TARGET_KINDS as kind (kind)}
              <option value={kind}>{KIND_LABEL[kind]()}</option>
            {/each}
          </select>
        </label>

        <label class="flex flex-col gap-2 text-body font-medium text-foreground">
          {report_target_id_label(undefined, { locale })}
          <input
            class={inputClass}
            inputmode="numeric"
            autocomplete="off"
            bind:value={targetId}
            aria-describedby="report-target-help"
          />
          <span id="report-target-help" class="text-caption text-muted">
            {report_target_id_help(undefined, { locale })}
          </span>
        </label>
      </div>

      <label class="flex flex-col gap-2 text-body font-medium text-foreground">
        {report_reason_label(undefined, { locale })}
        <select class={inputClass} bind:value={reason}>
          {#each REPORT_REASONS as r (r)}
            <option value={r}>{REASON_LABEL[r]()}</option>
          {/each}
        </select>
      </label>

      <label class="flex flex-col gap-2 text-body font-medium text-foreground">
        {report_details_label(undefined, { locale })}
        <textarea class={inputClass} rows="4" maxlength={REPORT_DETAILS_MAX} bind:value={details}
        ></textarea>
      </label>

      <label class="flex flex-col gap-2 text-body font-medium text-foreground">
        {report_contact_label(undefined, { locale })}
        <input class={inputClass} type="email" autocomplete="email" bind:value={contactEmail} />
      </label>

      <div class="flex flex-wrap items-center gap-4">
        <button
          type="submit"
          class="rounded-md bg-foreground px-5 py-2.5 text-body font-medium text-background disabled:opacity-60"
          disabled={status === "sending"}
        >
          {report_submit(undefined, { locale })}
        </button>
        <p role="status" aria-live="polite" class="text-body text-foreground-secondary">
          {statusMessage}
        </p>
      </div>
    </form>
  </div>
</Container>
