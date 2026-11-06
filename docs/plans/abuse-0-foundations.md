# Abuse phase 0: truth, measurement, ingress

[Back to index](../README.md)

#### A4b. Contact addresses

Needs MX proof first, so it waits for O3. Replace `corrections@easyquran.app` (`faq_script_answer`) and `salam@easyquran.app` (`legal_terms_contact_body`) in `web/messages/{en,ar}.json` with `privacy@`, `abuse@`, `legal@`, `support@` on `easyquran.fyi`. Add the `privacy@` address to the privacy page. Point `/contact` and `SITE.contactEmail` at `abuse@`/`support@`.

Done when: no `easyquran.app` address remains in messages; `/contact` no longer shows the owner's personal address.
Blocked by: O3
