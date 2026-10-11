import { literal, object, pipe, regex, safeParse, string } from "valibot";

const pendingProofSchema = object({
  status: literal("totp_required"),
  totp_token: pipe(string(), regex(/^[a-f0-9]{64}$/)),
});

// eslint-disable-next-line anti-slop/no-unknown-parameters -- data is the untrusted pending-proof HTTP response; this function validates its contract at the boundary.
export function pendingTotpToken(data: unknown): string | null {
  const parsed = safeParse(pendingProofSchema, data);
  if (!parsed.success) return null;
  return parsed.output.totp_token;
}
