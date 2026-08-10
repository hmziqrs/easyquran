import {
  authClient,
  decodeUserProfile,
  type AuthClient,
  type UserProfile,
} from "$lib/auth/auth-client";
import { authState } from "$lib/auth/auth-state.svelte";
import type { AuthTransitionContext } from "$lib/auth/auth-state.svelte";
import type { SessionProbeResult } from "$lib/auth/auth-client";
import {
  GENERIC_TRY_AGAIN,
  NETWORK_ERROR,
  VERIFY_EMAIL_NEXT,
  classifyAuthError,
  isVerifiedOnlyError,
} from "$lib/auth/auth-copy";
import type { AuthErrorEnvelope } from "$lib/auth/auth-client";

export interface PasskeyInfo {
  readonly id: string;
  readonly label: string;
  readonly deviceType?: string | null;
  readonly transports?: unknown;
  readonly createdAt?: string;
  readonly lastUsedAt?: string | null;
}

export interface PasskeyFlowStateLike {
  transition(ctx: AuthTransitionContext): Promise<void>;
  setUser(user: UserProfile | null): void;
  setTwoFaPending(pending: boolean): void;
  probe(): Promise<SessionProbeResult>;
}

export interface PasskeyFlowDeps {
  readonly client?: AuthClient;
  readonly state?: PasskeyFlowStateLike;
  readonly credentials?: CredentialsContainer;
}

interface CredentialView {
  readonly id: number;
  readonly credential_id: string;
  readonly device_type?: string | null;
  readonly transports?: unknown;
  readonly created_at?: string;
  readonly last_used_at?: string | null;
}

function b64uToBuffer(b64u: string): ArrayBuffer {
  const b64 = b64u.replace(/-/g, "+").replace(/_/g, "/");
  const pad = b64.length % 4 ? "=".repeat(4 - (b64.length % 4)) : "";
  const bin = atob(b64 + pad);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes.buffer;
}

function bufferToB64u(buf: ArrayBuffer | Uint8Array): string {
  const bytes = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function decodeBufferField(raw: unknown): ArrayBuffer | undefined {
  if (typeof raw === "string" && raw.length > 0) {
    try {
      return b64uToBuffer(raw);
    } catch {
      return undefined;
    }
  }
  return undefined;
}

function isUserCancellation(e: unknown): boolean {
  if (e instanceof DOMException) {
    return e.name === "AbortError" || e.name === "NotAllowedError";
  }
  return false;
}

function toPasskeyInfo(view: CredentialView): PasskeyInfo {
  return {
    id: view.credential_id,
    label: typeof view.device_type === "string" && view.device_type ? view.device_type : "Passkey",
    deviceType: view.device_type ?? null,
    transports: view.transports,
    createdAt: view.created_at,
    lastUsedAt: view.last_used_at ?? null,
  };
}

type NavigatorCredentials = CredentialsContainer | undefined;

function resolveCredentials(inject?: CredentialsContainer): NavigatorCredentials {
  if (inject) return inject;
  if (typeof navigator === "undefined") return undefined;
  return (navigator as unknown as { credentials?: CredentialsContainer }).credentials;
}

export class PasskeyFlow {
  readonly client: AuthClient;
  readonly state: PasskeyFlowStateLike;
  readonly #credentials: NavigatorCredentials;
  pending = $state(false);
  cancelled = $state(false);
  genericError = $state<string | null>(null);
  fieldErrors = $state<Readonly<Record<string, string>>>({});
  passkeys = $state<ReadonlyArray<PasskeyInfo>>([]);
  #loginOptions: PublicKeyCredentialRequestOptions | null = null;
  #registerOptions: PublicKeyCredentialCreationOptions | null = null;
  #loginState: unknown = null;
  #registerState: unknown = null;

  constructor(client: AuthClient, state: PasskeyFlowStateLike, credentials?: CredentialsContainer) {
    this.client = client;
    this.state = state;
    this.#credentials = resolveCredentials(credentials);
  }

  get supported(): boolean {
    return this.#credentials !== undefined;
  }

  clearSecrets(): void {
    this.#loginOptions = null;
    this.#registerOptions = null;
    this.#loginState = null;
    this.#registerState = null;
  }

  private fail(status: number, error: AuthErrorEnvelope | null): void {
    if (status === 0) {
      this.genericError = NETWORK_ERROR;
      this.fieldErrors = {};
      return;
    }
    const c = classifyAuthError(status, error, []);
    this.genericError = c.message;
    this.fieldErrors = {};
  }

  async login(): Promise<boolean> {
    if (this.pending) return false;
    if (!this.#credentials) {
      this.genericError = GENERIC_TRY_AGAIN;
      return false;
    }
    this.pending = true;
    this.cancelled = false;
    this.genericError = null;
    this.fieldErrors = {};
    try {
      const begin = await this.client.unsafeRequest<{
        challenge?: unknown;
        authentication_state?: unknown;
      }>("/passkey/v1/login/begin", { method: "POST" });
      if (!begin.ok) {
        this.fail(begin.status, begin.error);
        return false;
      }
      this.#loginState = begin.data?.authentication_state ?? null;
      const options = this.#toRequestOptions(begin.data?.challenge);
      if (!options) {
        this.genericError = GENERIC_TRY_AGAIN;
        return false;
      }
      this.#loginOptions = options;
      let assertion: PublicKeyCredential | null;
      try {
        assertion = (await this.#credentials.get({
          publicKey: options,
        } as CredentialRequestOptions)) as PublicKeyCredential | null;
      } catch (e) {
        if (isUserCancellation(e)) {
          this.cancelled = true;
          return false;
        }
        throw e;
      }
      this.#loginOptions = null;
      if (!assertion) {
        this.cancelled = true;
        return false;
      }
      const finish = await this.client.unsafeRequest<{ status?: string; user?: unknown }>(
        "/passkey/v1/login/finish",
        {
          method: "POST",
          body: {
            credential: this.#serializeAssertion(assertion),
            authentication_state: this.#loginState,
          },
        },
      );
      if (!finish.ok) {
        this.fail(finish.status, finish.error);
        return false;
      }
      const user = decodeUserProfile(finish.data?.user);
      if (!user) {
        this.genericError = GENERIC_TRY_AGAIN;
        return false;
      }
      if (!finish.rotated) {
        try {
          await this.client.refreshCsrf();
        } catch {}
      }
      await this.state.transition({ kind: "passkey" });
      this.state.setUser(user);
      this.state.setTwoFaPending(false);
      return true;
    } catch {
      this.genericError = NETWORK_ERROR;
      return false;
    } finally {
      this.pending = false;
      this.#loginOptions = null;
      this.#loginState = null;
    }
  }

  async register(label?: string): Promise<boolean> {
    if (this.pending) return false;
    if (!this.#credentials) {
      this.genericError = GENERIC_TRY_AGAIN;
      return false;
    }
    this.pending = true;
    this.cancelled = false;
    this.genericError = null;
    this.fieldErrors = {};
    try {
      const begin = await this.client.unsafeRequest<{
        challenge?: unknown;
        registration_state?: unknown;
      }>("/passkey/v1/register/begin", { method: "POST" });
      if (!begin.ok) {
        if (begin.status === 403 || isVerifiedOnlyError(begin.error)) {
          this.genericError = VERIFY_EMAIL_NEXT;
        } else {
          this.fail(begin.status, begin.error);
        }
        return false;
      }
      this.#registerState = begin.data?.registration_state ?? null;
      const options = this.#toCreationOptions(begin.data?.challenge);
      if (!options) {
        this.genericError = GENERIC_TRY_AGAIN;
        return false;
      }
      this.#registerOptions = options;
      let credential: PublicKeyCredential | null;
      try {
        credential = (await this.#credentials.create({
          publicKey: options,
        } as CredentialCreationOptions)) as PublicKeyCredential | null;
      } catch (e) {
        if (isUserCancellation(e)) {
          this.cancelled = true;
          return false;
        }
        throw e;
      }
      this.#registerOptions = null;
      if (!credential) {
        this.cancelled = true;
        return false;
      }
      const serialized = this.#serializeAttestation(credential);
      const finish = await this.client.unsafeRequest<CredentialView>(
        "/passkey/v1/register/finish",
        {
          method: "POST",
          body: {
            credential: serialized.credential,
            registration_state: this.#registerState,
            ...(serialized.transports ? { transports: serialized.transports } : {}),
            ...(label ? { device_type: label } : {}),
          },
        },
      );
      if (!finish.ok) {
        if (finish.status === 403 || isVerifiedOnlyError(finish.error)) {
          this.genericError = VERIFY_EMAIL_NEXT;
        } else {
          this.fail(finish.status, finish.error);
        }
        return false;
      }
      if (finish.data) {
        this.#mergeSingle(finish.data);
      } else {
        await this.list();
      }
      if (!finish.rotated) {
        try {
          await this.client.refreshCsrf();
        } catch {}
      }
      await this.state.transition({ kind: "passkey" });
      await this.state.probe();
      return true;
    } catch {
      this.genericError = NETWORK_ERROR;
      return false;
    } finally {
      this.pending = false;
      this.#registerOptions = null;
      this.#registerState = null;
    }
  }

  async list(): Promise<ReadonlyArray<PasskeyInfo> | null> {
    const res = await this.client.unsafeRequest<{ data?: unknown }>(
      "/passkey/v1/list",
      { method: "POST" },
    );
    if (!res.ok) {
      if (res.status === 401 || res.status === 403) return null;
      return null;
    }
    this.#mergeList(res.data?.data);
    return this.passkeys;
  }

  async remove(id: string): Promise<boolean> {
    if (!id) return false;
    const res = await this.client.unsafeRequest<{ message?: string }>(
      "/passkey/v1/remove",
      { method: "POST", body: { credential_id: id } },
    );
    if (!res.ok) {
      this.fail(res.status, res.error);
      return false;
    }
    this.passkeys = this.passkeys.filter((p) => p.id !== id);
    return true;
  }

  #mergeSingle(view: unknown): void {
    if (!view || typeof view !== "object") return;
    const v = view as Partial<CredentialView>;
    if (typeof v.credential_id !== "string") return;
    const info = toPasskeyInfo(v as CredentialView);
    const existing = this.passkeys.filter((p) => p.id !== info.id);
    this.passkeys = [...existing, info];
  }

  #mergeList(data: unknown): void {
    if (!Array.isArray(data)) return;
    const out: PasskeyInfo[] = [];
    for (const entry of data) {
      if (!entry || typeof entry !== "object") continue;
      const v = entry as Partial<CredentialView>;
      if (typeof v.credential_id !== "string") continue;
      out.push(toPasskeyInfo(v as CredentialView));
    }
    this.passkeys = out;
  }

  #toRequestOptions(challenge: unknown): PublicKeyCredentialRequestOptions | null {
    if (!challenge || typeof challenge !== "object") return null;
    const o = (challenge as { publicKey?: unknown }).publicKey;
    if (!o || typeof o !== "object") return null;
    const raw = o as Record<string, unknown>;
    const challengeBuf = decodeBufferField(raw.challenge);
    if (!challengeBuf) return null;
    const out: PublicKeyCredentialRequestOptions = {
      challenge: challengeBuf,
      ...(typeof raw.timeout === "number" ? { timeout: raw.timeout } : {}),
      ...(typeof raw.rpId === "string" ? { rpId: raw.rpId } : {}),
      ...(Array.isArray(raw.allowCredentials) ? { allowCredentials: this.#decodeCredentialDescriptors(raw.allowCredentials) } : {}),
      ...(typeof raw.userVerification === "string" ? { userVerification: raw.userVerification as UserVerificationRequirement } : {}),
    };
    return out;
  }

  #toCreationOptions(challenge: unknown): PublicKeyCredentialCreationOptions | null {
    if (!challenge || typeof challenge !== "object") return null;
    const o = (challenge as { publicKey?: unknown }).publicKey;
    if (!o || typeof o !== "object") return null;
    const raw = o as Record<string, unknown>;
    const challengeBuf = decodeBufferField(raw.challenge);
    if (!challengeBuf) return null;
    const userRaw = raw.user;
    if (!userRaw || typeof userRaw !== "object") return null;
    const u = userRaw as Record<string, unknown>;
    const userId = decodeBufferField(u.id);
    if (!userId) return null;
    const out: PublicKeyCredentialCreationOptions = {
      challenge: challengeBuf,
      rp: this.#decodeRp(raw.rp),
      user: {
        id: userId,
        name: typeof u.name === "string" ? u.name : "",
        displayName: typeof u.displayName === "string" ? u.displayName : "",
      },
      pubKeyCredParams: this.#decodeParams(raw.pubKeyCredParams),
      ...(Array.isArray(raw.excludeCredentials) ? { excludeCredentials: this.#decodeCredentialDescriptors(raw.excludeCredentials) } : {}),
      ...(Array.isArray(raw.authenticatorSelection) ? {} : typeof raw.authenticatorSelection === "object" && raw.authenticatorSelection ? { authenticatorSelection: raw.authenticatorSelection as AuthenticatorSelectionCriteria } : {}),
      ...(typeof raw.timeout === "number" ? { timeout: raw.timeout } : {}),
      ...(typeof raw.attestation === "string" ? { attestation: raw.attestation as AttestationConveyancePreference } : {}),
    };
    return out;
  }

  #decodeRp(raw: unknown): PublicKeyCredentialRpEntity {
    if (!raw || typeof raw !== "object") return { name: "EasyQuran" };
    const o = raw as Record<string, unknown>;
    return {
      name: typeof o.name === "string" ? o.name : "EasyQuran",
      ...(typeof o.id === "string" ? { id: o.id } : {}),
    };
  }

  #decodeParams(raw: unknown): PublicKeyCredentialParameters[] {
    if (!Array.isArray(raw)) return [{ type: "public-key", alg: -7 }];
    const out: PublicKeyCredentialParameters[] = [];
    for (const entry of raw) {
      if (!entry || typeof entry !== "object") continue;
      const o = entry as Record<string, unknown>;
      const type = o.type;
      const alg = o.alg;
      if (type === "public-key" && typeof alg === "number") out.push({ type: "public-key", alg });
    }
    return out.length > 0 ? out : [{ type: "public-key", alg: -7 }];
  }

  #decodeCredentialDescriptors(raw: unknown): PublicKeyCredentialDescriptor[] {
    if (!Array.isArray(raw)) return [];
    const out: PublicKeyCredentialDescriptor[] = [];
    for (const entry of raw) {
      if (!entry || typeof entry !== "object") continue;
      const o = entry as Record<string, unknown>;
      const id = decodeBufferField(o.id);
      if (!id) continue;
      out.push({
        type: "public-key",
        id,
        ...(Array.isArray(o.transports) ? { transports: o.transports as AuthenticatorTransport[] } : {}),
      });
    }
    return out;
  }

  #serializeAssertion(cred: PublicKeyCredential): Record<string, unknown> {
    const response = cred.response as AuthenticatorResponse & {
      authenticatorData?: ArrayBuffer;
      signature?: ArrayBuffer;
      userHandle?: ArrayBuffer | null;
      clientDataJSON: ArrayBuffer;
    };
    return {
      id: cred.id,
      rawId: bufferToB64u(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: bufferToB64u(response.clientDataJSON),
        ...(response.authenticatorData ? { authenticatorData: bufferToB64u(response.authenticatorData) } : {}),
        ...(response.signature ? { signature: bufferToB64u(response.signature) } : {}),
        ...(response.userHandle ? { userHandle: bufferToB64u(response.userHandle) } : {}),
      },
    };
  }

  #serializeAttestation(cred: PublicKeyCredential): {
    credential: Record<string, unknown>;
    transports: string[] | null;
  } {
    const response = cred.response as AuthenticatorAttestationResponse;
    let transports: string[] | null = null;
    if (typeof response.getTransports === "function") {
      try {
        const t = response.getTransports();
        if (Array.isArray(t) && t.length > 0) transports = t.slice();
      } catch {}
    }
    const credential: Record<string, unknown> = {
      id: cred.id,
      rawId: bufferToB64u(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: bufferToB64u(response.clientDataJSON),
        attestationObject: bufferToB64u(response.attestationObject),
        ...(transports ? { transports } : {}),
      },
    };
    return { credential, transports };
  }
}

export function createPasskeyFlow(deps: PasskeyFlowDeps = {}): PasskeyFlow {
  return new PasskeyFlow(
    deps.client ?? authClient,
    deps.state ?? authState,
    deps.credentials,
  );
}
