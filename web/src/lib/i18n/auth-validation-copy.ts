import {
  auth_error_backup_code_invalid,
  auth_error_code_digits,
  auth_error_code_length_8,
  auth_error_code_required,
  auth_error_confirm_mismatch,
  auth_error_confirm_required,
  auth_error_email_invalid,
  auth_error_email_required,
  auth_error_name_max,
  auth_error_name_required,
  auth_error_password_max,
  auth_error_password_min,
  auth_error_password_required,
} from "$lib/i18n/m/auth";
import type { UiLocale } from "$lib/i18n/locales";
import { getLocale } from "$lib/paraglide/runtime.js";

export interface AuthValidationCopy {
  nameRequired: string;
  nameMax: string;
  emailRequired: string;
  emailInvalid: string;
  passwordRequired: string;
  passwordMin: string;
  passwordMax: string;
  confirmRequired: string;
  confirmMismatch: string;
  codeRequired: string;
  codeDigits: string;
  codeLength8: string;
  backupCodeInvalid: string;
}

// SAFETY: paraglide is compiled for exactly the UI locales (en/ar in messages/), so getLocale() only ever returns a UiLocale at runtime.
export function getAuthValidationCopy(locale: UiLocale = getLocale() as UiLocale): AuthValidationCopy {
  return {
    nameRequired: auth_error_name_required(undefined, { locale }),
    nameMax: auth_error_name_max(undefined, { locale }),
    emailRequired: auth_error_email_required(undefined, { locale }),
    emailInvalid: auth_error_email_invalid(undefined, { locale }),
    passwordRequired: auth_error_password_required(undefined, { locale }),
    passwordMin: auth_error_password_min(undefined, { locale }),
    passwordMax: auth_error_password_max(undefined, { locale }),
    confirmRequired: auth_error_confirm_required(undefined, { locale }),
    confirmMismatch: auth_error_confirm_mismatch(undefined, { locale }),
    codeRequired: auth_error_code_required(undefined, { locale }),
    codeDigits: auth_error_code_digits(undefined, { locale }),
    codeLength8: auth_error_code_length_8(undefined, { locale }),
    backupCodeInvalid: auth_error_backup_code_invalid(undefined, { locale }),
  };
}
