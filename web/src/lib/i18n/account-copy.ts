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
  account_devices_heading,
  account_devices_description,
  account_devices_loading,
  account_sessions_empty,
  account_session_this,
  account_session_last_seen,
  account_session_ending,
  account_session_revoke,
  account_device_unknown,
  account_device_this,
  account_device_rename,
  account_device_rename_label,
  account_device_rename_save,
  account_device_rename_cancel,
  account_device_name_invalid,
  account_device_sign_out,
  account_device_signed_out,
  account_security_heading,
  account_passkeys_heading,
  account_passkeys_description,
  account_passkeys_unsupported,
  account_passkey_default_label,
  account_passkey_remove,
  account_passkey_adding,
  account_passkey_add,
  account_generic_error,
  account_delete_heading,
  account_delete_description,
  account_delete_export_hint,
  account_delete_export,
  account_delete_field_email,
  account_delete_mismatch,
  account_delete_fresh_auth,
  account_delete_submit,
  account_delete_deleting,
  account_delete_done,
  account_age_heading,
  account_age_description,
  account_age_field,
  account_age_submit,
  account_age_saving,
  account_age_invalid,
  account_age_done,
  account_age_already,
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
  devicesHeading: string;
  devicesDescription: string;
  devicesLoading: string;
  sessionsEmpty: string;
  sessionThis: string;
  sessionLastSeen: (date: string) => string;
  sessionEnding: string;
  sessionRevoke: string;
  deviceUnknown: string;
  deviceThis: string;
  deviceRename: string;
  deviceRenameLabel: string;
  deviceRenameSave: string;
  deviceRenameCancel: string;
  deviceNameInvalid: string;
  deviceSignOut: string;
  deviceSignedOut: (name: string) => string;
  securityHeading: string;
  passkeysHeading: string;
  passkeysDescription: string;
  passkeysUnsupported: string;
  passkeyDefaultLabel: string;
  passkeyRemove: string;
  passkeyAdding: string;
  passkeyAdd: string;
  genericError: string;
  deleteHeading: string;
  deleteDescription: string;
  deleteExportHint: string;
  deleteExport: string;
  deleteFieldEmail: string;
  deleteMismatch: string;
  deleteFreshAuth: string;
  deleteSubmit: string;
  deleteDeleting: string;
  deleteDone: string;
  ageHeading: string;
  ageDescription: string;
  ageField: string;
  ageSubmit: string;
  ageSaving: string;
  ageInvalid: string;
  ageDone: string;
  ageAlready: string;
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
    devicesHeading: account_devices_heading(undefined, { locale }),
    devicesDescription: account_devices_description(undefined, { locale }),
    devicesLoading: account_devices_loading(undefined, { locale }),
    sessionsEmpty: account_sessions_empty(undefined, { locale }),
    sessionThis: account_session_this(undefined, { locale }),
    sessionLastSeen: (date: string) => account_session_last_seen({ date }, { locale }),
    sessionEnding: account_session_ending(undefined, { locale }),
    sessionRevoke: account_session_revoke(undefined, { locale }),
    deviceUnknown: account_device_unknown(undefined, { locale }),
    deviceThis: account_device_this(undefined, { locale }),
    deviceRename: account_device_rename(undefined, { locale }),
    deviceRenameLabel: account_device_rename_label(undefined, { locale }),
    deviceRenameSave: account_device_rename_save(undefined, { locale }),
    deviceRenameCancel: account_device_rename_cancel(undefined, { locale }),
    deviceNameInvalid: account_device_name_invalid(undefined, { locale }),
    deviceSignOut: account_device_sign_out(undefined, { locale }),
    deviceSignedOut: (name: string) => account_device_signed_out({ name }, { locale }),
    securityHeading: account_security_heading(undefined, { locale }),
    passkeysHeading: account_passkeys_heading(undefined, { locale }),
    passkeysDescription: account_passkeys_description(undefined, { locale }),
    passkeysUnsupported: account_passkeys_unsupported(undefined, { locale }),
    passkeyDefaultLabel: account_passkey_default_label(undefined, { locale }),
    passkeyRemove: account_passkey_remove(undefined, { locale }),
    passkeyAdding: account_passkey_adding(undefined, { locale }),
    passkeyAdd: account_passkey_add(undefined, { locale }),
    genericError: account_generic_error(undefined, { locale }),
    deleteHeading: account_delete_heading(undefined, { locale }),
    deleteDescription: account_delete_description(undefined, { locale }),
    deleteExportHint: account_delete_export_hint(undefined, { locale }),
    deleteExport: account_delete_export(undefined, { locale }),
    deleteFieldEmail: account_delete_field_email(undefined, { locale }),
    deleteMismatch: account_delete_mismatch(undefined, { locale }),
    deleteFreshAuth: account_delete_fresh_auth(undefined, { locale }),
    deleteSubmit: account_delete_submit(undefined, { locale }),
    deleteDeleting: account_delete_deleting(undefined, { locale }),
    deleteDone: account_delete_done(undefined, { locale }),
    ageHeading: account_age_heading(undefined, { locale }),
    ageDescription: account_age_description(undefined, { locale }),
    ageField: account_age_field(undefined, { locale }),
    ageSubmit: account_age_submit(undefined, { locale }),
    ageSaving: account_age_saving(undefined, { locale }),
    ageInvalid: account_age_invalid(undefined, { locale }),
    ageDone: account_age_done(undefined, { locale }),
    ageAlready: account_age_already(undefined, { locale }),
  };
}
