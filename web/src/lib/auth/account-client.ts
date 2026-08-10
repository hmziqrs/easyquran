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
  readonly sessions: ReadonlyArray<SessionInfo>;
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

function decodeSession(raw: unknown): SessionInfo | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  if (typeof o.id !== "string") return null;
  const createdAt =
    typeof o.created_at === "string"
      ? o.created_at
      : typeof o.createdAt === "string"
        ? o.createdAt
        : undefined;
  const lastSeenAt =
    typeof o.last_seen_at === "string"
      ? o.last_seen_at
      : typeof o.lastSeenAt === "string"
        ? o.lastSeenAt
        : undefined;
  const userAgent =
    typeof o.user_agent === "string"
      ? o.user_agent
      : typeof o.userAgent === "string"
        ? o.userAgent
        : undefined;
  return {
    id: o.id,
    isCurrent: o.is_current === true || o.isCurrent === true,
    ...(createdAt !== undefined ? { createdAt } : {}),
    ...(lastSeenAt !== undefined ? { lastSeenAt } : {}),
    ...(userAgent !== undefined ? { userAgent } : {}),
  };
}

export function decodeSessionList(raw: unknown): SessionInfo[] {
  if (!raw || typeof raw !== "object") return [];
  const o = raw as Record<string, unknown>;
  const arr = Array.isArray(o.sessions) ? o.sessions : Array.isArray(raw) ? raw : [];
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
    const data = res.ok ? (res.data as Record<string, unknown> | null) : null;
    const isCurrent = !!(data && (data.is_current === true || data.isCurrent === true));
    return { status, httpStatus: res.status, isCurrent };
  }
}

export function createAccountClient(auth: AuthClient = authClient): AccountClient {
  return new AccountClient(auth);
}

export const accountClient: AccountClient = createAccountClient();
