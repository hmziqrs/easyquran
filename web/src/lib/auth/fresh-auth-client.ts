import * as v from "valibot";

import {
  authClient,
  isWireObject,
  type AuthClient,
  type AuthErrorEnvelope,
} from "#lib/auth/auth-client.js";

const statusSchema = v.object({
  ready: v.boolean(),
  has_password: v.boolean(),
  two_fa_enabled: v.boolean(),
});

export type FreshAuthStatus = v.InferOutput<typeof statusSchema>;

export function isFreshAuthenticationRequired(error: AuthErrorEnvelope | null): boolean {
  return isWireObject(error?.context) && error.context.reason === "fresh_auth_required";
}

export const FRESH_AUTH_REQUIRED =
  "Confirm your identity in Security confirmation, then try again.";

export class FreshAuthClient {
  constructor(readonly client: AuthClient = authClient) {}

  async status(): Promise<FreshAuthStatus | null> {
    const result = await this.client.get<FreshAuthStatus>("/auth/v1/reauth/status");
    if (!result.ok) return null;
    const parsed = v.safeParse(statusSchema, result.data);
    return parsed.success ? parsed.output : null;
  }

  async confirm(password: string, code?: string): Promise<string | null> {
    const result = await this.client.unsafeRequest<{ ready: boolean }>("/auth/v1/reauth", {
      method: "POST",
      body: { password, code: code || undefined },
    });
    if (result.ok && result.data?.ready === true) return null;
    if (result.status === 429) return "Too many attempts. Wait, then try again.";
    if (result.status === 400 || result.status === 401)
      return "Check your password and authentication code.";
    return "Confirmation failed. Try again.";
  }
}

export const freshAuthClient = new FreshAuthClient();
