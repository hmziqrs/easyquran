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
  readonly label?: string;
  readonly createdAt?: string;
  readonly lastUsedAt?: string;
}

export interface PasskeyListResponse {
  readonly passkeys: ReadonlyArray<PasskeyInfo>;
}

export interface PasskeyLoginBegin {
  readonly publicKey: unknown;
}

export interface PasskeyRegisterBegin {
  readonly publicKey: unknown;
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
      const begin = await this.client.unsafeRequest<PasskeyLoginBegin>(
        "/passkey/v1/login/begin",
        { method: "POST" },
      );
      if (!begin.ok) {
        this.fail(begin.status, begin.error);
        return false;
      }
      const options = this.#toRequestOptions(begin.data);
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
      const finish = await this.client.unsafeRequest<UserProfile>(
        "/passkey/v1/login/finish",
        { method: "POST", body: this.#serializeAssertion(assertion) },
      );
      if (!finish.ok) {
        this.fail(finish.status, finish.error);
        return false;
      }
      const user = decodeUserProfile(finish.data);
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
      const begin = await this.client.unsafeRequest<PasskeyRegisterBegin>(
        "/passkey/v1/register/begin",
        { method: "POST", body: label ? { label } : undefined },
      );
      if (!begin.ok) {
        if (begin.status === 403 || isVerifiedOnlyError(begin.error)) {
          this.genericError = VERIFY_EMAIL_NEXT;
        } else {
          this.fail(begin.status, begin.error);
        }
        return false;
      }
      const options = this.#toCreationOptions(begin.data);
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
      const finish = await this.client.unsafeRequest<PasskeyListResponse>(
        "/passkey/v1/register/finish",
        { method: "POST", body: this.#serializeAttestation(credential) },
      );
      if (!finish.ok) {
        if (finish.status === 403 || isVerifiedOnlyError(finish.error)) {
          this.genericError = VERIFY_EMAIL_NEXT;
        } else {
          this.fail(finish.status, finish.error);
        }
        return false;
      }
      this.#mergeList(finish.data);
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
    }
  }

  async list(): Promise<ReadonlyArray<PasskeyInfo> | null> {
    const res = await this.client.get<PasskeyListResponse>("/passkey/v1/list");
    if (!res.ok) {
      if (res.status === 401 || res.status === 403) return null;
      return null;
    }
    this.#mergeList(res.data);
    return this.passkeys;
  }

  async remove(id: string): Promise<boolean> {
    if (!id) return false;
    const res = await this.client.unsafeRequest<PasskeyListResponse>(
      "/passkey/v1/remove",
      { method: "POST", body: { id } },
    );
    if (!res.ok) {
      this.fail(res.status, res.error);
      return false;
    }
    this.#mergeList(res.data);
    return true;
  }

  #mergeList(data: PasskeyListResponse | null): void {
    if (!data) return;
    const arr = Array.isArray(data.passkeys) ? data.passkeys : [];
    this.passkeys = arr.filter((p): p is PasskeyInfo => {
      if (!p || typeof p !== "object") return false;
      return typeof (p as { id?: unknown }).id === "string";
    });
  }

  #toRequestOptions(data: unknown): PublicKeyCredentialRequestOptions | null {
    if (!data || typeof data !== "object") return null;
    const o = (data as { publicKey?: unknown }).publicKey;
    if (!o || typeof o !== "object") return null;
    const raw = o as Record<string, unknown>;
    const challenge = decodeBufferField(raw.challenge);
    if (!challenge) return null;
    const out: PublicKeyCredentialRequestOptions = {
      challenge,
      ...(typeof raw.timeout === "number" ? { timeout: raw.timeout } : {}),
      ...(typeof raw.rpId === "string" ? { rpId: raw.rpId } : {}),
      ...(Array.isArray(raw.allowCredentials) ? { allowCredentials: this.#decodeCredentialDescriptors(raw.allowCredentials) } : {}),
      ...(typeof raw.userVerification === "string" ? { userVerification: raw.userVerification as UserVerificationRequirement } : {}),
    };
    return out;
  }

  #toCreationOptions(data: unknown): PublicKeyCredentialCreationOptions | null {
    if (!data || typeof data !== "object") return null;
    const o = (data as { publicKey?: unknown }).publicKey;
    if (!o || typeof o !== "object") return null;
    const raw = o as Record<string, unknown>;
    const challenge = decodeBufferField(raw.challenge);
    if (!challenge) return null;
    const userRaw = raw.user;
    if (!userRaw || typeof userRaw !== "object") return null;
    const u = userRaw as Record<string, unknown>;
    const userId = decodeBufferField(u.id);
    if (!userId) return null;
    const out: PublicKeyCredentialCreationOptions = {
      challenge,
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
      raw_id: bufferToB64u(cred.rawId),
      type: cred.type,
      response: {
        client_data_json: bufferToB64u(response.clientDataJSON),
        ...(response.authenticatorData ? { authenticator_data: bufferToB64u(response.authenticatorData) } : {}),
        ...(response.signature ? { signature: bufferToB64u(response.signature) } : {}),
        ...(response.userHandle ? { user_handle: bufferToB64u(response.userHandle) } : {}),
      },
    };
  }

  #serializeAttestation(cred: PublicKeyCredential): Record<string, unknown> {
    const response = cred.response as AuthenticatorAttestationResponse;
    return {
      id: cred.id,
      raw_id: bufferToB64u(cred.rawId),
      type: cred.type,
      response: {
        client_data_json: bufferToB64u(response.clientDataJSON),
        attestation_object: bufferToB64u(response.attestationObject),
      },
    };
  }
}

export function createPasskeyFlow(deps: PasskeyFlowDeps = {}): PasskeyFlow {
  return new PasskeyFlow(
    deps.client ?? authClient,
    deps.state ?? authState,
    deps.credentials,
  );
}
