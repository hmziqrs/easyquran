import { uiDirection, type UiDirection, type UiLocale } from "#lib/i18n/locales.js";
import {
  auth_create_account,
  auth_confirm_password_placeholder,
  auth_create_one_link,
  auth_dialog_description,
  auth_dialog_title,
  auth_display_name_placeholder,
  auth_email_placeholder,
  auth_field_authentication_code,
  auth_field_confirm_password,
  auth_field_display_name,
  auth_field_email,
  auth_field_password,
  auth_forgot_password_prompt,
  auth_have_account_prompt,
  auth_hide_password_aria,
  auth_more_sign_in_options_aria,
  auth_no_account_prompt,
  auth_oauth_error,
  auth_captcha_error,
  auth_captcha_label,
  auth_captcha_required,
  auth_or_continue_with,
  auth_password_min_placeholder,
  auth_password_placeholder,
  auth_please_wait,
  auth_register_heading,
  auth_register_subheading,
  auth_reset_password_link,
  auth_show_password_aria,
  auth_sign_in,
  auth_sign_in_subheading,
  auth_totp_heading,
  auth_totp_subheading,
  auth_use_different_account,
  auth_continue,
  auth_continue_with_passkey,
  auth_continue_with_provider,
  auth_close,
  auth_connected_accounts_heading,
  auth_connected_accounts_description,
  auth_connected_linked_on,
  auth_connected_unlink,
  auth_connected_unlink_confirm,
  auth_connected_unlink_cancel,
  auth_connected_last_method_hint,
  auth_connected_link_provider,
  auth_connected_linked_banner,
  auth_connected_already_linked_error,
  auth_connected_cancelled_error,
  auth_connected_failed_error,
  auth_connected_unlink_error,
} from "#lib/i18n/m/auth.js";
import { getLocale } from "#lib/paraglide/runtime.js";

export interface AuthCopy {
  locale: UiLocale;
  direction: UiDirection;
  dialogTitle: string;
  dialogDescription: string;
  close: string;
  signIn: string;
  createAccount: string;
  pleaseWait: string;
  totpHeading: string;
  totpSubheading: string;
  continue: string;
  fieldAuthenticationCode: string;
  useDifferentAccount: string;
  signInSubheading: string;
  fieldEmail: string;
  emailPlaceholder: string;
  fieldPassword: string;
  passwordPlaceholder: string;
  forgotPasswordPrompt: string;
  resetPasswordLink: string;
  noAccountPrompt: string;
  createOneLink: string;
  registerHeading: string;
  registerSubheading: string;
  fieldDisplayName: string;
  displayNamePlaceholder: string;
  passwordMinPlaceholder: string;
  confirmPasswordPlaceholder: string;
  fieldConfirmPassword: string;
  haveAccountPrompt: string;
  oauthError: string;
  captchaLabel: string;
  captchaRequired: string;
  captchaError: string;
  moreSignInOptionsAria: string;
  orContinueWith: string;
  continueWithProvider: (provider: string) => string;
  continueWithPasskey: string;
  showPasswordAria: string;
  hidePasswordAria: string;
  connectedAccountsHeading: string;
  connectedAccountsDescription: string;
  connectedLinkedOn: (date: string) => string;
  connectedUnlink: string;
  connectedUnlinkConfirm: string;
  connectedUnlinkCancel: string;
  connectedLastMethodHint: string;
  connectedLinkProvider: (provider: string) => string;
  connectedLinkedBanner: (provider: string) => string;
  connectedAlreadyLinkedError: (provider: string) => string;
  connectedCancelledError: string;
  connectedFailedError: string;
  connectedUnlinkError: string;
}

// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getAuthCopy(locale: UiLocale = getLocale() as UiLocale): AuthCopy {
  return {
    locale,
    direction: uiDirection(locale),
    dialogTitle: auth_dialog_title(undefined, { locale }),
    dialogDescription: auth_dialog_description(undefined, { locale }),
    close: auth_close(undefined, { locale }),
    signIn: auth_sign_in(undefined, { locale }),
    createAccount: auth_create_account(undefined, { locale }),
    pleaseWait: auth_please_wait(undefined, { locale }),
    totpHeading: auth_totp_heading(undefined, { locale }),
    totpSubheading: auth_totp_subheading(undefined, { locale }),
    continue: auth_continue(undefined, { locale }),
    fieldAuthenticationCode: auth_field_authentication_code(undefined, { locale }),
    useDifferentAccount: auth_use_different_account(undefined, { locale }),
    signInSubheading: auth_sign_in_subheading(undefined, { locale }),
    fieldEmail: auth_field_email(undefined, { locale }),
    emailPlaceholder: auth_email_placeholder(undefined, { locale }),
    fieldPassword: auth_field_password(undefined, { locale }),
    passwordPlaceholder: auth_password_placeholder(undefined, { locale }),
    forgotPasswordPrompt: auth_forgot_password_prompt(undefined, { locale }),
    resetPasswordLink: auth_reset_password_link(undefined, { locale }),
    noAccountPrompt: auth_no_account_prompt(undefined, { locale }),
    createOneLink: auth_create_one_link(undefined, { locale }),
    registerHeading: auth_register_heading(undefined, { locale }),
    registerSubheading: auth_register_subheading(undefined, { locale }),
    fieldDisplayName: auth_field_display_name(undefined, { locale }),
    displayNamePlaceholder: auth_display_name_placeholder(undefined, { locale }),
    passwordMinPlaceholder: auth_password_min_placeholder(undefined, { locale }),
    confirmPasswordPlaceholder: auth_confirm_password_placeholder(undefined, { locale }),
    fieldConfirmPassword: auth_field_confirm_password(undefined, { locale }),
    haveAccountPrompt: auth_have_account_prompt(undefined, { locale }),
    oauthError: auth_oauth_error(undefined, { locale }),
    captchaLabel: auth_captcha_label(undefined, { locale }),
    captchaRequired: auth_captcha_required(undefined, { locale }),
    captchaError: auth_captcha_error(undefined, { locale }),
    moreSignInOptionsAria: auth_more_sign_in_options_aria(undefined, { locale }),
    orContinueWith: auth_or_continue_with(undefined, { locale }),
    continueWithProvider: (provider: string) =>
      auth_continue_with_provider({ provider }, { locale }),
    continueWithPasskey: auth_continue_with_passkey(undefined, { locale }),
    showPasswordAria: auth_show_password_aria(undefined, { locale }),
    hidePasswordAria: auth_hide_password_aria(undefined, { locale }),
    connectedAccountsHeading: auth_connected_accounts_heading(undefined, { locale }),
    connectedAccountsDescription: auth_connected_accounts_description(undefined, { locale }),
    connectedLinkedOn: (date: string) => auth_connected_linked_on({ date }, { locale }),
    connectedUnlink: auth_connected_unlink(undefined, { locale }),
    connectedUnlinkConfirm: auth_connected_unlink_confirm(undefined, { locale }),
    connectedUnlinkCancel: auth_connected_unlink_cancel(undefined, { locale }),
    connectedLastMethodHint: auth_connected_last_method_hint(undefined, { locale }),
    connectedLinkProvider: (provider: string) =>
      auth_connected_link_provider({ provider }, { locale }),
    connectedLinkedBanner: (provider: string) =>
      auth_connected_linked_banner({ provider }, { locale }),
    connectedAlreadyLinkedError: (provider: string) =>
      auth_connected_already_linked_error({ provider }, { locale }),
    connectedCancelledError: auth_connected_cancelled_error(undefined, { locale }),
    connectedFailedError: auth_connected_failed_error(undefined, { locale }),
    connectedUnlinkError: auth_connected_unlink_error(undefined, { locale }),
  };
}
