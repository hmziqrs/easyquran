import { uiDirection, type UiDirection, type UiLocale } from "#lib/i18n/locales.js";
import {
  account_page_title,
  account_checking_session,
  account_loading_heading,
  account_loading_body,
  account_retry,
  account_heading,
  account_anonymous_body,
  account_sign_in,
  account_create_account,
  account_signed_in_body,
  account_sign_out,
  account_signing_out,
  account_profile_heading,
  account_field_name,
  account_field_email,
  account_name_empty,
  account_profile_saved,
  account_save_changes,
  account_saving,
  account_sessions_heading,
  account_sessions_description,
  account_sessions_loading,
  account_sessions_empty,
  account_session_this,
  account_session_last_seen,
  account_session_ending,
  account_session_revoke,
  account_device_unknown,
  account_security_heading,
  account_passkeys_heading,
  account_passkeys_description,
  account_passkeys_unsupported,
  account_passkey_default_label,
  account_passkey_remove,
  account_passkey_adding,
  account_passkey_add,
  account_generic_error,
} from "#lib/i18n/m/account.js";
import { getLocale } from "#lib/paraglide/runtime.js";

export interface AccountCopy {
  locale: UiLocale;
  direction: UiDirection;
  pageTitle: string;
  checkingSession: string;
  loadingHeading: string;
  loadingBody: string;
  retry: string;
  heading: string;
  anonymousBody: string;
  signIn: string;
  createAccount: string;
  signedInBody: string;
  signOut: string;
  signingOut: string;
  profileHeading: string;
  fieldName: string;
  fieldEmail: string;
  nameEmpty: string;
  profileSaved: string;
  saveChanges: string;
  saving: string;
  sessionsHeading: string;
  sessionsDescription: string;
  sessionsLoading: string;
  sessionsEmpty: string;
  sessionThis: string;
  sessionLastSeen: (date: string) => string;
  sessionEnding: string;
  sessionRevoke: string;
  deviceUnknown: string;
  securityHeading: string;
  passkeysHeading: string;
  passkeysDescription: string;
  passkeysUnsupported: string;
  passkeyDefaultLabel: string;
  passkeyRemove: string;
  passkeyAdding: string;
  passkeyAdd: string;
  genericError: string;
}

// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getAccountCopy(locale: UiLocale = getLocale() as UiLocale): AccountCopy {
  return {
    locale,
    direction: uiDirection(locale),
    pageTitle: account_page_title(undefined, { locale }),
    checkingSession: account_checking_session(undefined, { locale }),
    loadingHeading: account_loading_heading(undefined, { locale }),
    loadingBody: account_loading_body(undefined, { locale }),
    retry: account_retry(undefined, { locale }),
    heading: account_heading(undefined, { locale }),
    anonymousBody: account_anonymous_body(undefined, { locale }),
    signIn: account_sign_in(undefined, { locale }),
    createAccount: account_create_account(undefined, { locale }),
    signedInBody: account_signed_in_body(undefined, { locale }),
    signOut: account_sign_out(undefined, { locale }),
    signingOut: account_signing_out(undefined, { locale }),
    profileHeading: account_profile_heading(undefined, { locale }),
    fieldName: account_field_name(undefined, { locale }),
    fieldEmail: account_field_email(undefined, { locale }),
    nameEmpty: account_name_empty(undefined, { locale }),
    profileSaved: account_profile_saved(undefined, { locale }),
    saveChanges: account_save_changes(undefined, { locale }),
    saving: account_saving(undefined, { locale }),
    sessionsHeading: account_sessions_heading(undefined, { locale }),
    sessionsDescription: account_sessions_description(undefined, { locale }),
    sessionsLoading: account_sessions_loading(undefined, { locale }),
    sessionsEmpty: account_sessions_empty(undefined, { locale }),
    sessionThis: account_session_this(undefined, { locale }),
    sessionLastSeen: (date: string) => account_session_last_seen({ date }, { locale }),
    sessionEnding: account_session_ending(undefined, { locale }),
    sessionRevoke: account_session_revoke(undefined, { locale }),
    deviceUnknown: account_device_unknown(undefined, { locale }),
    securityHeading: account_security_heading(undefined, { locale }),
    passkeysHeading: account_passkeys_heading(undefined, { locale }),
    passkeysDescription: account_passkeys_description(undefined, { locale }),
    passkeysUnsupported: account_passkeys_unsupported(undefined, { locale }),
    passkeyDefaultLabel: account_passkey_default_label(undefined, { locale }),
    passkeyRemove: account_passkey_remove(undefined, { locale }),
    passkeyAdding: account_passkey_adding(undefined, { locale }),
    passkeyAdd: account_passkey_add(undefined, { locale }),
    genericError: account_generic_error(undefined, { locale }),
  };
}
