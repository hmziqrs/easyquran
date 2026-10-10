import {
  authClient,
  decodeUserProfile,
  isNumber,
  isString,
  isWireObject,
  type AuthClient,
  type AuthErrorEnvelope,
  type JsonValue,
  type UserProfile,
  type WireObject,
} from "#lib/auth/auth-client.js";

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

export type IdentityProvider = "github" | "discord" | "google";

const IDENTITY_PROVIDERS: ReadonlySet<string> = new Set<IdentityProvider>([
  "github",
  "discord",
  "google",
]);

export function isIdentityProvider(value: string): value is IdentityProvider {
  return IDENTITY_PROVIDERS.has(value);
}

export interface LinkedIdentity {
  readonly provider: IdentityProvider;
  readonly providerUserId: string;
  readonly createdAt: string;
}

export interface IdentitySummary {
  readonly identities: ReadonlyArray<LinkedIdentity>;
  readonly hasPassword: boolean;
  readonly passkeyCount: number;
  readonly canUnlinkAny: boolean;
  readonly linkableProviders: ReadonlyArray<IdentityProvider>;
}

const IDENTITY_PROVIDER_LABELS = {
  github: "GitHub",
  discord: "Discord",
  google: "Google",
} satisfies Record<IdentityProvider, string>;

export function identityProviderLabel(provider: IdentityProvider): string {
  return IDENTITY_PROVIDER_LABELS[provider];
}

// Sign-in method count: every linked identity + the password (if set) + the
// passkey factor (if at least one exists). The server's can_unlink_any mirrors
// this rule; the UI recomputes it so the disable state never lags the counts.
export function countSignInMethods(summary: IdentitySummary): number {
  const passkey = summary.passkeyCount > 0 ? 1 : 0;
  return summary.identities.length + (summary.hasPassword ? 1 : 0) + passkey;
}

export function canUnlinkIdentity(summary: IdentitySummary): boolean {
  return countSignInMethods(summary) > 1;
}

export type AccountResultStatus = "ok" | "anonymous" | "error";

type SessionDraft = {
  id: string;
  isCurrent: boolean;
  createdAt?: string;
  lastSeenAt?: string;
  userAgent?: string;
};

type ProfileUpdateWire = {
  name?: string;
  avatar_id?: number | null;
};

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
function firstString(source: WireObject, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = source[key];
    if (isString(value)) return value;
  }
  return undefined;
}

// id is the numeric i32 audit-row id on the wire; tolerate a numeric string
// (never require a string id — the backend sends a number).
function decodeSessionId(value: JsonValue | undefined): string | null {
  if (isNumber(value)) return String(value);
  if (isString(value) && value.length > 0) return value;
  return null;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is one unparsed row from the sessions JSON body; this function is its boundary parser
function decodeSession(raw: unknown): SessionInfo | null {
  if (!isWireObject(raw)) return null;
  const o = raw;
  const id = decodeSessionId(o.id);
  if (id === null) return null;
  // Backend row fields: device, last_seen (snake_case) + isCurrent (camelCase,
  // server-derived). Tolerate optional camelCase variants for robustness.
  const lastSeenAt = firstString(o, "last_seen", "lastSeenAt");
  const userAgent = firstString(o, "device", "user_agent", "userAgent");
  const createdAt = firstString(o, "created_at", "createdAt");
  const session: SessionDraft = { id, isCurrent: o.is_current === true || o.isCurrent === true };
  if (createdAt !== undefined) session.createdAt = createdAt;
  if (lastSeenAt !== undefined) session.lastSeenAt = lastSeenAt;
  if (userAgent !== undefined) session.userAgent = userAgent;
  return session;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the still-unparsed sessions payload (may be a bare array); each row is parsed by decodeSession below
function sessionRows(raw: unknown, envelope: WireObject): unknown[] {
  if (Array.isArray(envelope.data)) return envelope.data;
  if (Array.isArray(raw)) return raw;
  return [];
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed JSON body from fetch; this function is the I/O boundary parser for the sessions list
export function decodeSessionList(raw: unknown): SessionInfo[] {
  if (!isWireObject(raw)) return [];
  const o = raw;
  // Real envelope is { data: [...], total, page }. Accept a bare array too.
  const arr = sessionRows(raw, o);
  const out: SessionInfo[] = [];
  for (const entry of arr) {
    const s = decodeSession(entry);
    if (s) out.push(s);
  }
  return out;
}

function decodeLinkedIdentity(raw: JsonValue | undefined): LinkedIdentity | null {
  if (!isWireObject(raw)) return null;
  const o = raw;
  const provider = o.provider;
  if (!isString(provider) || !isIdentityProvider(provider)) return null;
  const providerUserId = firstString(o, "provider_user_id", "providerUserId");
  if (providerUserId === undefined) return null;
  return {
    provider,
    providerUserId,
    createdAt: firstString(o, "created_at", "createdAt") ?? "",
  };
}

function decodeLinkableProviders(raw: JsonValue | undefined): IdentityProvider[] {
  const out: IdentityProvider[] = [];
  if (!Array.isArray(raw)) return out;
  for (const entry of raw) {
    if (!isString(entry) || !isIdentityProvider(entry)) continue;
    if (!out.includes(entry)) out.push(entry);
  }
  return out;
}

// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed identities JSON body from fetch; this function is its boundary parser
export function decodeIdentitySummary(raw: unknown): IdentitySummary | null {
  if (!isWireObject(raw)) return null;
  const o = raw;
  const rawIdentities = o.identities;
  if (!Array.isArray(rawIdentities)) return null;
  const identities: LinkedIdentity[] = [];
  for (const entry of rawIdentities) {
    const identity = decodeLinkedIdentity(entry);
    if (identity) identities.push(identity);
  }
  return {
    identities,
    hasPassword: o.has_password === true,
    passkeyCount: isNumber(o.passkey_count) ? o.passkey_count : 0,
    canUnlinkAny: o.can_unlink_any === true,
    linkableProviders: decodeLinkableProviders(o.linkable_providers),
  };
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
    const body: ProfileUpdateWire = {};
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

  async getIdentities(): Promise<AccountResult<IdentitySummary>> {
    const res = await this.auth.get<WireObject>("/user/v1/identities");
    const status = statusFor(res.status);
    const data = res.ok ? decodeIdentitySummary(res.data) : null;
    return { status, httpStatus: res.status, data, error: res.error };
  }

  async unlinkIdentity(provider: IdentityProvider): Promise<AccountResult<IdentitySummary>> {
    const res = await this.auth.unsafeRequest<WireObject>("/user/v1/identities/unlink", {
      method: "POST",
      body: { provider },
    });
    const status = statusFor(res.status);
    const data = res.ok ? decodeIdentitySummary(res.data) : null;
    return { status, httpStatus: res.status, data, error: res.error };
  }
}

export function createAccountClient(auth: AuthClient = authClient): AccountClient {
  return new AccountClient(auth);
}

export const accountClient: AccountClient = createAccountClient();
