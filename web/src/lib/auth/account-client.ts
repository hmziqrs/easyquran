import {
  authClient,
  decodeUserProfile,
  type AuthClient,
  type AuthErrorEnvelope,
  type UserProfile,
} from "$lib/auth/auth-client";

export interface SessionInfo {
  readonly id: string;
  readonly isCurrent: boolean;
  readonly createdAt?: string;
  readonly lastSeenAt?: string;
  readonly userAgent?: string;
}

export interface SessionListResponse {
  readonly data: ReadonlyArray<SessionInfo>;
  readonly total?: number;
  readonly page?: number;
}

export interface ProfileUpdateInput {
  readonly name?: string;
  readonly avatarId?: number | null;
}

export type AccountResultStatus = "ok" | "anonymous" | "error";

export interface AccountResult<T> {
  readonly status: AccountResultStatus;
  readonly httpStatus: number;
  readonly data: T | null;
  readonly error: AuthErrorEnvelope | null;
}

export interface TerminateResult {
  readonly status: AccountResultStatus;
  readonly httpStatus: number;
  readonly isCurrent: boolean;
}

/** First key present as a string, in preference order. Wire rows spell fields snake_case or camelCase. */
function firstString(source: Record<string, unknown>, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === "string") return value;
  }
  return undefined;
}

// id is the numeric i32 audit-row id on the wire; tolerate a numeric string
// (never require a string id — the backend sends a number).
function decodeSessionId(value: unknown): string | null {
  if (typeof value === "number") return String(value);
  if (typeof value === "string" && value.length > 0) return value;
  return null;
}

function decodeSession(raw: unknown): SessionInfo | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  const id = decodeSessionId(o.id);
  if (id === null) return null;
  // Backend row fields: device, last_seen (snake_case) + isCurrent (camelCase,
  // server-derived). Tolerate optional camelCase variants for robustness.
  const lastSeenAt = firstString(o, "last_seen", "lastSeenAt");
  const userAgent = firstString(o, "device", "user_agent", "userAgent");
  const createdAt = firstString(o, "created_at", "createdAt");
  return {
    id,
    isCurrent: o.is_current === true || o.isCurrent === true,
    ...(createdAt !== undefined ? { createdAt } : {}),
    ...(lastSeenAt !== undefined ? { lastSeenAt } : {}),
    ...(userAgent !== undefined ? { userAgent } : {}),
  };
}

function sessionRows(raw: unknown, envelope: Record<string, unknown>): unknown[] {
  if (Array.isArray(envelope.data)) return envelope.data;
  if (Array.isArray(raw)) return raw;
  return [];
}

export function decodeSessionList(raw: unknown): SessionInfo[] {
  if (!raw || typeof raw !== "object") return [];
  const o = raw as Record<string, unknown>;
  // Real envelope is { data: [...], total, page }. Accept a bare array too.
  const arr = sessionRows(raw, o);
  const out: SessionInfo[] = [];
  for (const entry of arr) {
    const s = decodeSession(entry);
    if (s) out.push(s);
  }
  return out;
}

function statusFor(httpStatus: number): AccountResultStatus {
  if (httpStatus === 401 || httpStatus === 403) return "anonymous";
  if (httpStatus >= 200 && httpStatus < 300) return "ok";
  return "error";
}

export class AccountClient {
  readonly auth: AuthClient;

  constructor(auth: AuthClient = authClient) {
    this.auth = auth;
  }

  async updateProfile(input: ProfileUpdateInput): Promise<AccountResult<UserProfile>> {
    const body: Record<string, unknown> = {};
    if (input.name !== undefined) body.name = input.name;
    if (input.avatarId !== undefined) body.avatar_id = input.avatarId;
    const res = await this.auth.unsafeRequest<UserProfile>("/user/v1/update", {
      method: "POST",
      body,
    });
    const status = statusFor(res.status);
    const data = res.ok ? decodeUserProfile(res.data) : null;
    return { status, httpStatus: res.status, data, error: res.error };
  }

  async listSessions(): Promise<AccountResult<SessionInfo[]>> {
    const res = await this.auth.get<SessionListResponse>("/auth/v1/sessions/list");
    const status = statusFor(res.status);
    const data = res.ok ? decodeSessionList(res.data) : null;
    return { status, httpStatus: res.status, data, error: res.error };
  }

  async terminateSession(id: string): Promise<TerminateResult> {
    const res = await this.auth.unsafeRequest<unknown>(
      `/auth/v1/sessions/terminate/${encodeURIComponent(id)}`,
      { method: "DELETE" },
    );
    const status = statusFor(res.status);
    return { status, httpStatus: res.status, isCurrent: false };
  }
}

export function createAccountClient(auth: AuthClient = authClient): AccountClient {
  return new AccountClient(auth);
}

export const accountClient: AccountClient = createAccountClient();
