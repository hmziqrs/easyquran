import {
  authClient,
  decodeUserProfile,
  isNumber,
  isString,
  isWireObject,
  type AuthClient,
  type JsonValue,
  type UserProfile,
} from "$lib/auth/auth-client";
import type { SessionProbeResult } from "$lib/auth/auth-client";
import type { AuthErrorEnvelope } from "$lib/auth/auth-client";
import {
  GENERIC_TRY_AGAIN,
  NETWORK_ERROR,
  VERIFY_EMAIL_NEXT,
  classifyAuthError,
  isVerifiedOnlyError,
} from "$lib/auth/auth-copy";
import { authState } from "$lib/auth/auth-state.svelte";
import type { AuthTransitionContext } from "$lib/auth/auth-state.svelte";

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
  readonly transports?: JsonValue;
  readonly created_at?: string;
  readonly last_used_at?: string | null;
}

interface AuthenticatorSelectionEnvelope {
  authenticatorSelection?: AuthenticatorSelectionCriteria;
}

interface SerializedCredentialResponse {
  clientDataJSON: string;
  attestationObject?: string;
  authenticatorData?: string;
  signature?: string;
  userHandle?: string;
  transports?: string[];
}

interface SerializedCredential {
  id: string;
  rawId: string;
  type: string;
  response: SerializedCredentialResponse;
}

interface AttestationPayload {
  credential: SerializedCredential;
  transports: string[] | null;
}

interface RegisterFinishBody {
  credential: SerializedCredential;
  registration_state: JsonValue | null;
  transports?: string[];
  device_type?: string;
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
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function decodeBufferField(raw: JsonValue | undefined): ArrayBuffer | undefined {
  if (isString(raw) && raw.length > 0) {
    try {
      return b64uToBuffer(raw);
    } catch {
      return undefined;
    }
  }
  return undefined;
}

function isUserCancellation(cause: unknown): boolean {
  if (cause instanceof DOMException) {
    return cause.name === "AbortError" || cause.name === "NotAllowedError";
  }
  return false;
}

function toPasskeyInfo(view: CredentialView): PasskeyInfo {
  const deviceType = view.device_type;
  return {
    id: view.credential_id,
    label: isString(deviceType) && deviceType ? deviceType : "Passkey",
    deviceType: view.device_type ?? null,
    transports: view.transports,
    createdAt: view.created_at,
    lastUsedAt: view.last_used_at ?? null,
  };
}

type NavigatorCredentials = CredentialsContainer | undefined;

/**
 * Server options may omit authenticatorSelection entirely. An array is never a valid value for it,
 * so it is dropped rather than forwarded to the authenticator.
 */
// eslint-disable-next-line anti-slop/no-unknown-parameters -- raw is the unparsed authenticatorSelection JSON field; a narrower input type would assume the very wire contract this check exists to verify
function authenticatorSelectionOf(raw: unknown): AuthenticatorSelectionEnvelope {
  if (Array.isArray(raw) || !isWireObject(raw)) return {};
  // SAFETY: isWireObject ruled out arrays, null and primitives; the surviving server JSON object is the WebAuthn selection criteria.
  return { authenticatorSelection: raw as AuthenticatorSelectionCriteria };
}

function resolveCredentials(inject?: CredentialsContainer): NavigatorCredentials {
  if (inject) return inject;
  // eslint-disable-next-line anti-slop/no-runtime-typeof -- navigator is an ambient global that is undeclared in some runtimes; typeof is the only ReferenceError-safe presence check
  if (typeof navigator === "undefined") return undefined;
  // SAFETY: where navigator exists it is the standard Navigator, which always exposes .credentials.
  return (navigator as { credentials?: CredentialsContainer }).credentials;
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
  #loginState: JsonValue | null = null;
  #registerState: JsonValue | null = null;

  constructor(client: AuthClient, state: PasskeyFlowStateLike, credentials?: CredentialsContainer) {
    this.client = client;
    this.state = state;
    this.#credentials = resolveCredentials(credentials);
  }

  get supported(): boolean {
    return this.#credentials !== undefined;
  }

  clearSecrets(): void {
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
        challenge?: JsonValue;
        authentication_state?: JsonValue;
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
      let assertion: PublicKeyCredential | null;
      try {
        // SAFETY: credentials.get() with a publicKey request resolves to PublicKeyCredential | null
        // per WebAuthn; the literal already structurally satisfies CredentialRequestOptions.
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
        challenge?: JsonValue;
        registration_state?: JsonValue;
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
      let credential: PublicKeyCredential | null;
      try {
        // SAFETY: credentials.create() with a publicKey creation request resolves to
        // PublicKeyCredential | null per WebAuthn; the literal already structurally satisfies
        // CredentialCreationOptions.
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
      if (!credential) {
        this.cancelled = true;
        return false;
      }
      const serialized = this.#serializeAttestation(credential);
      const body: RegisterFinishBody = {
        credential: serialized.credential,
        registration_state: this.#registerState,
      };
      if (serialized.transports) body.transports = serialized.transports;
      if (label) body.device_type = label;
      const finish = await this.client.unsafeRequest<CredentialView>(
        "/passkey/v1/register/finish",
        {
          method: "POST",
          body,
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
      this.#registerState = null;
    }
  }

  async list(): Promise<ReadonlyArray<PasskeyInfo> | null> {
    const res = await this.client.unsafeRequest<{ data?: JsonValue }>("/passkey/v1/list", {
      method: "POST",
    });
    if (!res.ok) {
      if (res.status === 401 || res.status === 403) return null;
      return null;
    }
    this.#mergeList(res.data?.data);
    return this.passkeys;
  }

  async remove(id: string): Promise<boolean> {
    if (!id) return false;
    const res = await this.client.unsafeRequest<{ message?: string }>("/passkey/v1/remove", {
      method: "POST",
      body: { credential_id: id },
    });
    if (!res.ok) {
      this.fail(res.status, res.error);
      return false;
    }
    this.passkeys = this.passkeys.filter((p) => p.id !== id);
    return true;
  }

  // eslint-disable-next-line anti-slop/no-unknown-parameters -- view is untrusted register-finish JSON; this method is the shape check (isWireObject + isString below), so no honest narrower type exists at entry
  #mergeSingle(view: unknown): void {
    if (!isWireObject(view)) return;
    // SAFETY: isWireObject proved a JSON object; Partial reflects that server fields may still be absent.
    const v = view as Partial<CredentialView>;
    if (!isString(v.credential_id)) return;
    // SAFETY: credential_id was just checked to be a string; the finish endpoint returns full rows.
    const info = toPasskeyInfo(v as CredentialView);
    const existing = this.passkeys.filter((p) => p.id !== info.id);
    this.passkeys = [...existing, info];
  }

  // eslint-disable-next-line anti-slop/no-unknown-parameters -- data is the raw passkey-list payload; the loop below is the parse step, entries stay opaque until isWireObject/isString narrow them
  #mergeList(data: unknown): void {
    if (!Array.isArray(data)) return;
    const out: PasskeyInfo[] = [];
    for (const entry of data) {
      if (!isWireObject(entry)) continue;
      // SAFETY: isWireObject proved a JSON object; Partial reflects that server fields may still be absent.
      const v = entry as Partial<CredentialView>;
      if (!isString(v.credential_id)) continue;
      // SAFETY: credential_id was just checked to be a string; the list endpoint returns full rows.
      out.push(toPasskeyInfo(v as CredentialView));
    }
    this.passkeys = out;
  }

  #toRequestOptions(challenge: JsonValue | undefined): PublicKeyCredentialRequestOptions | null {
    if (!isWireObject(challenge)) return null;
    const raw = challenge.publicKey;
    if (!isWireObject(raw)) return null;
    const challengeBuf = decodeBufferField(raw.challenge);
    if (!challengeBuf) return null;
    const out: PublicKeyCredentialRequestOptions = {
      challenge: challengeBuf,
    };
    const timeout = raw.timeout;
    if (isNumber(timeout)) out.timeout = timeout;
    const rpId = raw.rpId;
    if (isString(rpId)) out.rpId = rpId;
    if (Array.isArray(raw.allowCredentials)) {
      out.allowCredentials = this.#decodeCredentialDescriptors(raw.allowCredentials);
    }
    const userVerification = raw.userVerification;
    if (isString(userVerification)) {
      // SAFETY: server-sent userVerification is a UserVerificationRequirement enum value; the
      // authenticator ignores values outside the enum.
      out.userVerification = userVerification as UserVerificationRequirement;
    }
    return out;
  }

  #toCreationOptions(challenge: JsonValue | undefined): PublicKeyCredentialCreationOptions | null {
    if (!isWireObject(challenge)) return null;
    const raw = challenge.publicKey;
    if (!isWireObject(raw)) return null;
    const challengeBuf = decodeBufferField(raw.challenge);
    if (!challengeBuf) return null;
    const u = raw.user;
    if (!isWireObject(u)) return null;
    const userId = decodeBufferField(u.id);
    if (!userId) return null;
    const userName = u.name;
    const userDisplayName = u.displayName;
    const out: PublicKeyCredentialCreationOptions = {
      challenge: challengeBuf,
      rp: this.#decodeRp(raw.rp),
      user: {
        id: userId,
        name: isString(userName) ? userName : "",
        displayName: isString(userDisplayName) ? userDisplayName : "",
      },
      pubKeyCredParams: this.#decodeParams(raw.pubKeyCredParams),
    };
    if (Array.isArray(raw.excludeCredentials)) {
      out.excludeCredentials = this.#decodeCredentialDescriptors(raw.excludeCredentials);
    }
    const selection = authenticatorSelectionOf(raw.authenticatorSelection);
    if (selection.authenticatorSelection) {
      out.authenticatorSelection = selection.authenticatorSelection;
    }
    const timeout = raw.timeout;
    if (isNumber(timeout)) out.timeout = timeout;
    const attestation = raw.attestation;
    if (isString(attestation)) {
      // SAFETY: server-sent attestation is an AttestationConveyancePreference enum value; the
      // browser ignores values outside the enum.
      out.attestation = attestation as AttestationConveyancePreference;
    }
    return out;
  }

  #decodeRp(raw: JsonValue | undefined): PublicKeyCredentialRpEntity {
    if (!isWireObject(raw)) return { name: "EasyQuran" };
    const rpName = raw.name;
    const out: PublicKeyCredentialRpEntity = {
      name: isString(rpName) ? rpName : "EasyQuran",
    };
    const rpId = raw.id;
    if (isString(rpId)) out.id = rpId;
    return out;
  }

  #decodeParams(raw: JsonValue | undefined): PublicKeyCredentialParameters[] {
    if (!Array.isArray(raw)) return [{ type: "public-key", alg: -7 }];
    const out: PublicKeyCredentialParameters[] = [];
    for (const entry of raw) {
      if (!isWireObject(entry)) continue;
      const type = entry.type;
      const alg = entry.alg;
      if (type === "public-key" && isNumber(alg)) out.push({ type: "public-key", alg });
    }
    return out.length > 0 ? out : [{ type: "public-key", alg: -7 }];
  }

  #decodeCredentialDescriptors(raw: JsonValue | undefined): PublicKeyCredentialDescriptor[] {
    if (!Array.isArray(raw)) return [];
    const out: PublicKeyCredentialDescriptor[] = [];
    for (const entry of raw) {
      if (!isWireObject(entry)) continue;
      const id = decodeBufferField(entry.id);
      if (!id) continue;
      const descriptor: PublicKeyCredentialDescriptor = {
        type: "public-key",
        id,
      };
      const transports = entry.transports;
      if (Array.isArray(transports)) {
        // SAFETY: server transports entries are WebAuthn transport-name strings; the browser
        // ignores values outside the enum.
        descriptor.transports = transports as AuthenticatorTransport[];
      }
      out.push(descriptor);
    }
    return out;
  }

  #serializeAssertion(cred: PublicKeyCredential): SerializedCredential {
    // SAFETY: get() with a publicKey request resolves to an assertion credential, so response
    // carries clientDataJSON plus the assertion-only authenticatorData/signature/userHandle buffers.
    const response = cred.response as AuthenticatorResponse & {
      authenticatorData?: ArrayBuffer;
      signature?: ArrayBuffer;
      userHandle?: ArrayBuffer | null;
      clientDataJSON: ArrayBuffer;
    };
    const serialized: SerializedCredentialResponse = {
      clientDataJSON: bufferToB64u(response.clientDataJSON),
    };
    if (response.authenticatorData) {
      serialized.authenticatorData = bufferToB64u(response.authenticatorData);
    }
    if (response.signature) {
      serialized.signature = bufferToB64u(response.signature);
    }
    if (response.userHandle) {
      serialized.userHandle = bufferToB64u(response.userHandle);
    }
    return {
      id: cred.id,
      rawId: bufferToB64u(cred.rawId),
      type: cred.type,
      response: serialized,
    };
  }

  #serializeAttestation(cred: PublicKeyCredential): AttestationPayload {
    // SAFETY: create() with a publicKey creation request resolves to an attestation credential, so
    // response is AuthenticatorAttestationResponse (attestationObject buffer + getTransports()).
    const response = cred.response as AuthenticatorAttestationResponse;
    let transports: string[] | null = null;
    // eslint-disable-next-line anti-slop/no-runtime-typeof -- runtime feature detection: older browsers ship AuthenticatorAttestationResponse without getTransports
    if (typeof response.getTransports === "function") {
      try {
        const t = response.getTransports();
        if (Array.isArray(t) && t.length > 0) transports = t.slice();
      } catch {}
    }
    const credential: SerializedCredential = {
      id: cred.id,
      rawId: bufferToB64u(cred.rawId),
      type: cred.type,
      response: {
        clientDataJSON: bufferToB64u(response.clientDataJSON),
        attestationObject: bufferToB64u(response.attestationObject),
      },
    };
    if (transports) {
      credential.response.transports = transports;
    }
    return { credential, transports };
  }
}

export function createPasskeyFlow(deps: PasskeyFlowDeps = {}): PasskeyFlow {
  return new PasskeyFlow(deps.client ?? authClient, deps.state ?? authState, deps.credentials);
}
