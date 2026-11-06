# Moderation SOP: account bans and multi-account decisions

Operator checklist for an account ban (ladder L6). The API enforces the parts marked
**server**. The rest are **operator**: the server cannot see them, so the operator
records them in the case notes and the audit row.

Reference: `docs/research/abuse-prevention/R2-ban-operations.md`.

## Precedence

Account ban > grant > challenge > nothing. A device signal is advisory: it can
challenge, never ban. A device-triggered action caps at L2. (**server**: these rules
exist as pure functions in `admin_user_bans_v1/precedence.rs`. No route calls them yet,
so the operator applies them when deciding.)

## Checklist for a multi-account decision

Work through the steps in order. Stop at the first step that rules the account out.

1. **Prior case link.** Name the earlier case or ban this account is linked to. No
   link, no multi-account decision.
2. **Functional test.** Can the second account do something the first account is
   barred from doing? If not, it is not evasion. Stop.
3. **Causal test.** Was the second account created after the ban, on a device or
   network retained from that ban? If not, it is not evasion. Stop.
4. **Admissible corroborators.** Find at least one of: credential reuse, identical
   violating content, or a declared link between the accounts. A device match alone
   is not a corroborator. (**server**: `corroborator` is required on every ban, and
   `none` is recorded when nothing was found.)
5. **Shared-device check (mandatory).** Check whether the device or network is shared
   (family phone, madrasa lab, internet cafe, kiosk registration). If it is shared:
   - step the action down one level (L6 becomes L5 at most);
   - never create an account ban on this basis.
   (**server**: `sharedDeviceChecked` must be `true` on create, or the request gets a
   400. The server records that the step was run. It cannot see the finding, so this
   step is the operator's responsibility.)
6. **Continuity.** Does the account look like a person continuing their own use (the
   same reading trail, reading hours and language), or a script replaying actions?
   Record which you saw.
7. **Closed reason codes.** Pick exactly one: `spam`, `harassment`,
   `credential_abuse`, `policy_violation`, `other`. (**server**: the API accepts only
   these codes.)

## After the decision

- Record the case note: the steps above, the corroborator, and the reason code.
- A notice is sent to the account's email at ban create. Check its `deliveryState`
  in the response. `sent` means the provider accepted it. `suppressed`,
  `no_contact_details` and `send_failed` need follow-up. Use the resend action
  after fixing the cause.
- Ban length is at most 365 days and always expires. Extending a ban needs a written
  self-review note (**server**: `renew` requires `singleOperatorControl` and a note).
- Revoke a ban the moment the evidence no longer supports it. The revoke is audited.
