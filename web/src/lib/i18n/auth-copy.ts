import { getLocale } from "$lib/paraglide/runtime.js";
import {
  auth_create_account,
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
} from "$lib/i18n/m/auth";

export interface AuthCopy {
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
  fieldConfirmPassword: string;
  haveAccountPrompt: string;
  oauthError: string;
  moreSignInOptionsAria: string;
  orContinueWith: string;
  continueWithProvider: (provider: string) => string;
  continueWithPasskey: string;
  showPasswordAria: string;
  hidePasswordAria: string;
}

export function getAuthCopy(locale = getLocale()): AuthCopy {
  return {
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
    fieldConfirmPassword: auth_field_confirm_password(undefined, { locale }),
    haveAccountPrompt: auth_have_account_prompt(undefined, { locale }),
    oauthError: auth_oauth_error(undefined, { locale }),
    moreSignInOptionsAria: auth_more_sign_in_options_aria(undefined, { locale }),
    orContinueWith: auth_or_continue_with(undefined, { locale }),
    continueWithProvider: (provider: string) => auth_continue_with_provider({ provider }, { locale }),
    continueWithPasskey: auth_continue_with_passkey(undefined, { locale }),
    showPasswordAria: auth_show_password_aria(undefined, { locale }),
    hidePasswordAria: auth_hide_password_aria(undefined, { locale }),
  };
}
