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

## Statement of reasons (Art 17 template)

Send with every ban, restriction, comment hide or content removal. Every field is
required. A field left blank means do not send yet. Store the sent text in the case
note.

```
Decision:        [account ban | account restriction | comment hidden | comment removed]
Decided at (UTC): [YYYY-MM-DD HH:MM]
Ground:          [closed reason code: spam | harassment | credential_abuse | policy_violation | other]
Terms section:   [section number and sentence in the Terms, section 5 "Acceptable use"]
Facts relied on: [one short paragraph. Name no other user. No device or network identifiers.]
Automated:       No decision is made by automated means alone. A device or network signal
                 can only challenge, never ban (precedence rules, this document).
Effect:          [what stops, from when]
Ends:            [YYYY-MM-DD, at most 365 days after the decision; or "until revoked"]
Appeal:          You can appeal this decision through the appeal form on the site
                 (appeal intake, A35). Give the reference below.
Reference:       [case or notice id]
Redress:         [owner text, O11. Left blank until the owner signs it.]
```

Operator checks before sending: the facts match the case note; the reason code is
one of the five closed codes; the notice `deliveryState` is `sent` (see After the
decision above).

## Appeal handling

Operator procedure for an appeal received through the intake (A35, `appeal_v1`).

1. Read the original decision, its notice, and the evidence on the case. Do not
   read anything the appellant did not submit beyond the case.
2. Check the shared-device step again if the decision used one. A shared device
   cannot support an account ban.
3. Decide: uphold, reverse, or change the effect. Write the reason in the case note.
4. Reverse a ban by revoke. The revoke is audited.
5. Send the decision to the appellant using the same template fields above, with
   the reason and the outcome.

**Clock.** Internal target is 7 days (`DECISION_CLOCK_SECS`). The code records
the appeal but does not enforce that clock, and no API records the decision yet.
Decisions live in the case notes until `legal-debt.md` row 7 closes. Do not publish
a seven-day promise until then.
