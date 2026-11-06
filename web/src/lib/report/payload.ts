/**
 * Report intake (A24). Builds the POST body for /report/v1/submit. Pure: no I/O,
 * so the route and the tests share one normaliser. The server enforces the same
 * limits (closed reason set, positive integer id, 2000-char details).
 */

export const REPORT_TARGET_KINDS = ["post", "comment"] as const;
export type ReportTargetKind = (typeof REPORT_TARGET_KINDS)[number];

export const REPORT_REASONS = [
  "spam",
  "harassment",
  "hate",
  "sexual",
  "violence",
  "copyright",
  "csam",
  "threat",
  "other",
] as const;
export type ReportReason = (typeof REPORT_REASONS)[number];

export const REPORT_DETAILS_MAX = 2000;

export interface ReportFormInput {
  readonly targetKind: ReportTargetKind;
  readonly targetId: string;
  readonly reason: ReportReason;
  readonly details: string;
  readonly contactEmail: string;
}

/** Wire body for POST /report/v1/submit. Optional fields are set only when present. */
export interface ReportBody {
  target_kind: ReportTargetKind;
  target_id: string;
  reason: ReportReason;
  details?: string;
  contact_email?: string;
}

export type ReportBuild =
  | { readonly ok: true; readonly body: ReportBody }
  | { readonly ok: false; readonly error: "invalid_target" | "details_too_long" };

const TARGET_ID = /^[1-9]\d{0,9}$/;

function trimmedOrUndefined(raw: string): string | undefined {
  const value = raw.trim();
  return value === "" ? undefined : value;
}

/** Normalise the form. Blank optional fields are omitted from the body. */
export function buildReportPayload(input: ReportFormInput): ReportBuild {
  const targetId = input.targetId.trim();
  if (!TARGET_ID.test(targetId)) return { ok: false, error: "invalid_target" };

  const details = trimmedOrUndefined(input.details);
  if (details !== undefined && details.length > REPORT_DETAILS_MAX) {
    return { ok: false, error: "details_too_long" };
  }

  const contactEmail = trimmedOrUndefined(input.contactEmail)?.toLowerCase();

  const body: ReportBody = {
    target_kind: input.targetKind,
    target_id: targetId,
    reason: input.reason,
  };
  if (details !== undefined) body.details = details;
  if (contactEmail !== undefined) body.contact_email = contactEmail;
  return { ok: true, body };
}
