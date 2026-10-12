# R2 — Legal & privacy constraints on security-only browser/device fingerprinting

**Deliverable for:** design of a security-only fingerprinting system (auth hardening, fraud/abuse prevention, ban enforcement) for a small, non-commercial Quran web app with a global audience.
**Prepared:** October 2026. **Method:** primary sources fetched directly (statutes, DPA guidance, regulator decisions, vendor docs). Where a claim is my analysis rather than a quoted source, it is marked `[analysis]`. Where I could not verify, it is marked `[unverified]` and listed in §10.
**Not legal advice.** This is a research input for a design decision; a lawyer should review the final privacy policy and the retention schedule before launch.

---

## 0. The single most important structural fact

There are **two separate legal questions** that get conflated, and they have different answers:

1. **ePrivacy/PECR "terminal equipment" question** — may you _read/write_ the device at all without consent? (EU: Art 5(3) Directive 2002/58/EC; UK: PECR reg 6 + Sch A1.)
2. **Data-protection question** — once you have the identifier, may you _process_ it, for what, how long, with what rights? (GDPR / UK GDPR / CCPA / LGPD / DPDP / APPI / KVKK / PDPL.)

A security-only deployment can pass (1) via the "strictly necessary" carve-out and still fail (2) — e.g. by keeping the identifier forever, or by reusing it for analytics. Conversely, some regimes (notably India) make (1) and (2) collapse into "consent is the only basis", which changes the design.

**Second structural fact:** the app is a _Quran_ service. In the EU, UK, Brazil and California, **religious or philosophical belief is a sensitive/special category** (GDPR Art 9(1); LGPD Art 5(II) _"convicção religiosa"_; CCPA §1798.140(ae)(1)(D)). Merely being a user of the app is data that can reveal religious belief. That is the highest-severity risk in this whole design, and it is not solved by hashing the fingerprint. See §6.7.

---

## 1. ePrivacy Directive Art 5(3) — does fingerprinting count as "access to information stored in terminal equipment"?

### 1.1 Yes. It has been settled since 2014 and reconfirmed by the EDPB in 2024.

**EDPB Guidelines 2/2023 on the technical scope of Art 5(3) ePrivacy Directive, Version 2.0, adopted 7 October 2024** (final; v1.0 was the November 2023 consultation draft). Note the numbering: **2/2023**, not 5/2023.

Verbatim, ¶2:

> "Article 29 Working Party ... Opinion 9/2014 on the application of ePrivacy Directive to device fingerprinting ... has already clarified that **fingerprinting falls within the technical scope of Article 5(3) ePD**, but due to the new advances in technologies further guidance is needed with respect to the tracking techniques currently observed."

¶3 explains _why_ the guidance exists:

> "The ambiguities regarding the scope of application of Article 5(3) ePD have created incentives to implement alternative solutions for tracking internet users and lead to a tendency to circumvent the legal obligations provided by Article 5(3) ePD."

The Guidelines set out three criteria (¶6): (A) the operation relates to **"information"** — "It should be noted that the term used is not 'personal data', but 'information'"; (B) it involves a **terminal equipment** of a subscriber/user (including a device merely _connectable_ to a public communications network, even if currently disconnected — ¶15); (C) it constitutes **storage** or **gaining of access** (¶¶6, 30–34).

### 1.2 The CJEU authority: Planet49 (C-673/17) — and how the EDPB uses it

The EDPB quotes the CJEU at ¶10 of the Guidelines:

> "That protection applies to **any information stored in such terminal equipment, regardless of whether or not it is personal data**, and is intended, in particular, as is clear from that recital, to protect users from the risk that hidden identifiers and other similar devices enter those users' terminal equipment without their knowledge."
> — citing _Planet49 GmbH v Bundesverband der Verbraucherzentralen_, C-673/17, judgment of 1 October 2019, ECLI:EU:C:2019:801, **paragraph 70**.

So the Art 5(3) question is **not** a personal-data question. Even a purely technical device identifier is in scope.

Planet49's other holdings matter for the consent-UI section (§4): consent must be given by **active behaviour**, a pre-ticked checkbox is not valid consent, and consent must be specific and informed. `[verified via EDPB Guidelines 2/2023 ¶10 + CJEU citation; I could not fetch the eur-lex HTML of the judgment in this session — EUR-Lex blocked server-side fetches, so the paragraph text is quoted through the EDPB's own citation of ¶70.]`

**Later CJEU cases to know (none of them narrow Art 5(3)):**

- **C-604/22, IAB Europe** (7 March 2024) — a TC String (consent string) is personal data and ePrivacy Art 5(3) applies to the storage/access it depends on. `[cited from memory of the ruling; not fetched in this session — treat as orientation, verify before quoting]`
- **C-446/21, Schrems v Meta Platforms Ireland** (4 Oct 2024) — Art 5(1)(c) data minimisation precludes unlimited aggregation/analysis over time for advertising; a public statement can make special-category data "manifestly made public" under Art 9(2)(e). Directly relevant to §6.7 (religious belief) and §3 (retention). `[same caveat]`
- **C-252/21, Meta v Bundeskartellamt** (4 July 2023) — relied on by the EDPB in its legitimate-interest guidelines for the necessity/data-minimisation test in security processing. Cited verbatim inside the EDPB LI guidelines I did fetch.
- **C-582/14, Breyer** (19 Oct 2016) — dynamic IP addresses for ensuring general operability of online media fall under the legitimate-interest balancing test. Cited verbatim by the EDPB in the same guidelines.

### 1.3 The "strictly necessary" exemption — what it actually says

Art 5(3) ePD, second sentence, quoted verbatim in the EDPB Guidelines (footnote 4 to ¶8):

> "This shall not prevent any technical storage or access for the sole purpose of carrying out the transmission of a communication over an electronic communications network, or **as strictly necessary in order for the provider of an information society service explicitly requested by the subscriber or user to provide the service**."

Two things to notice, and they are the crux of the EU analysis:

1. The EU text **does not enumerate security, fraud prevention or authentication** as exempt purposes. The UK, post-Brexit, does (see §1.6). The EU relies on Member State transposition + DPA interpretation of "strictly necessary ... to provide the service".
2. The EDPB **deliberately refused to opine on the exemption**. Guidelines 2/2023 ¶4:

> "These Guidelines do not address the circumstances under which a processing operation may fall within the exemptions from the consent requirement provided for by the ePD, as these circumstances should be analysed on a **case-by-case basis accounting for the relevant member state transposition(s), and guidance issued by national Competent Authorities**."

Repeated at ¶40 and ¶56: "the applicability of this article does not systematically mean that consent needs to be collected."

**Consequence for the design:** the strength of the security exemption is **not uniform across the EU/EEA**. It is strongest in the UK (explicit statute), moderately strong in France/Spain/Netherlands where DPAs have written about security purposes, and it must be checked per Member State if you ever have a real user base there. For a small app the pragmatic posture is: _assume you may rely on strictly necessary for security in the EU, but write the analysis down and never reuse the identifier._

### 1.4 CNIL: guidance, and the advertising/enforcement picture

- **Definition.** CNIL's glossary entry: fingerprinting is _"une technique probabiliste visant à identifier un utilisateur de façon unique sur un site web ou une application mobile en utilisant les caractéristiques techniques de son navigateur"_, and CNIL notes cookie-blocking tools do **not** stop it: "Les mécanismes de gestion ou blocage des cookies ne permettent pas de s'opposer à cette technique."
- **CNIL's own lab (LINC) explicitly frames fingerprinting as a security tool.** LINC article "Fingerprinting", by Romain Darous, published 2 June 2026: "This technique is very useful for filtering a website's web traffic and **blocking bots** that may connect to it for scraping purposes or to carry out attacks such as DDoS, for example." The article documents exactly the technique a small app would use — **JA4+ TCP/TLS/HTTP fingerprints** computed at the transport/security layer before application data is decrypted, plus HTTP header analysis (User-Agent, Accept-Language, If-Modified-Since) and browser fingerprinting via client-side JS. Crucially it notes TCP/TLS-layer fingerprints "can also be handled by third-party services such as content delivery networks (CDNs) or cloud-based web application firewalls", whereas HTTP/browser-level analysis "must therefore be performed on the server hosting the website." LINC also links FingerprintJS's `BotD` library as a bot-detection implementation.
- **CNIL on advertising fingerprinting (Feb 2025).** When Google enabled fingerprinting for advertisers, CNIL's position, as reported: _"L'usage du fingerprinting à des fins publicitaires nécessite le consentement des utilisateurs qui doit pouvoir refuser aussi simplement que d'accepter."_ `[secondary-source quote via BFMTV, Feb 2025 — I did not locate a CNIL-published page containing this sentence in this session; treat the wording as reported, not as an official citation.]` The substance is consistent with CNIL's cookie doctrine.
- **CNIL enforcement.** CNIL's cookies/trackers programme is its most prolific sanctions theme; its 2025 sanctions review lists cookies, employee surveillance and data security as the dominant topics. CNIL publishes the full register at `cnil.fr/fr/thematique/cnil/sanctions` and `cnil.fr/fr/les-sanctions-prononcees-par-la-cnil`. `[I verified the register URLs exist in search results and the CNIL cookies-guidelines page; I did not open each individual 2022/2024 fingerprinting decision. Any claim of a specific "CNIL 2022 fingerprinting fine" should be re-checked against the register before it is cited externally.]`
- **CNIL cookie doctrine (transposition).** CNIL's sheet on legal basis: "an exception is made when cookies are for the sole purpose of carrying out electronic communication, or are **strictly necessary to provide a service requested by the user**. Furthermore, **the use of a single tracer for multiple purposes does not exempt from obtaining consent for the purposes that require it.**" That last sentence is the operative rule for our design: one identifier, one purpose.

### 1.5 ICO / EDPB statements

- **EDPB Cookie Banner Taskforce report** (18 January 2023) — the enforcement-consistency reference on banner design (equal prominence of reject, no misleading colours, no cookie walls). EDPB document index: `edpb.europa.eu/documents/task-force-report/report-of-the-work-undertaken-by-the-cookie-banner-taskforce_en`.
- **EDPB Guidelines 3/2025 on the interplay between the DSA and the GDPR** — final version published 17 September 2026 (v1.0 was the Sept–Oct 2025 consultation). Relevant to §7. Plus an EDPB factsheet "GDPR and DSA, how they interact". `[I fetched the consultation page and confirmed the final version exists with a Sept 2026 file path; I did not read the final text.]`
- **EDPB/EDPS Joint Opinion 2/2026 on the Digital Omnibus** (11 February 2026) — the Board's position on the Commission's simplification package, tagged under "Automated decision making, profiling and Online tracking". `[confirmed to exist; not read.]`

### 1.6 UK — a materially different and _better_ answer than the EU (verify before relying on it)

This is the most significant jurisdiction divergence in the report, and it favours the UK design.

**Data (Use and Access) Act 2025, s.112** replaced PECR reg 6 and inserted reg 6A + a new **Schedule A1**. Commencement: s.112 was in force at Royal Assent for specified purposes and fully in force **5 February 2026** (S.I. 2026/82, reg. 2(w)). Schedule A1 was inserted **5.2.2026**.

Schedule A1, paragraph 4(2) — verbatim:

> "For the purposes of this paragraph, the technical storage of, or technical access to, information is **strictly necessary** for the provision of an information society service requested by the subscriber or user if, for example, the storage or access is strictly necessary—
> (a) to protect information provided in connection with, or relating to, the provision of the service requested,
> (b) to ensure that the security of the terminal equipment of the subscriber or user is not adversely affected by the provision of the service requested,
> (c) **to prevent or detect fraud in connection with the provision of the service requested**,
> (d) to prevent or detect technical faults in connection with the provision of the service requested, or
> (e) to enable either of the following things to be done where necessary for the provision of the service requested—
> (i) **automatically authenticating the identity of the subscriber or user**, or
> (ii) maintaining a record of selections made on a website, or information put into a website, by the subscriber or user."

Also: Sch A1 ¶3 (transmission exception); ¶2 (consent definition, incl. "amending or setting controls on the internet browser" as a means of signifying consent — a UK-specific oddity); ¶5 statistical purposes (with mandatory "simple means of objecting"); ¶6 website appearance; ¶7 emergency assistance. Reg 6A gives the Secretary of State power to add/remove exceptions by affirmative SI — so this list can change.

**ICO final guidance on Storage and Access Technologies (SATs), published 29 April 2026** (finalised after two consultations: the Dec 2024 cookies-guidance update and the July 2025 PECR/DUAA consultation). Its "What are the exceptions?" chapter is unusually explicit for our purposes:

- Five exceptions: communication; strictly necessary; statistical purposes; appearance; emergency assistance.
- **Communication exception table:** "**Device fingerprinting techniques, solely for network management purposes.**" → ✔ meets the exception.
- **Strictly necessary — PECR's non-exhaustive examples of qualifying activities:** "ensuring the security of terminal equipment; **preventing or detecting fraud**; preventing or detecting technical faults; **authenticating the subscriber or user**; and recording information or selections the user makes on an online service."
- **The killer rule for design:** "if you say your use of a particular technology is strictly necessary because of the purpose (eg security), you **must** ensure that you only use it for this purpose. **If you use it for any other purpose as well, the exception does not apply** and you must then get consent."
- **Advertising:** "You **must** also get consent if you are using **device fingerprinting techniques for online advertising purposes**." And: "Online advertising purposes are not exempt from PECR's consent requirements and never have been."
- **Cross-site/cross-device:** "You must get consent for any cross-site or cross-device tracking you want to do."
- **Also useful:** the "statistical purposes" exception permits logs/recordings of individual visitors only "if not obtained for the purposes of security" — i.e. security logging sits in the security exception, not the analytics exception.
- ICO further reading links a separate page **"Profiling tools for online safety"** (ICO, UK GDPR guidance) — worth reading before you build any abuse-scoring, because it addresses the interaction between safety profiling and data-protection law.

**Bottom line for UK:** a security-only, same-site, non-advertising device fingerprint used to prevent fraud and authenticate users is **expressly within the statutory exception**, provided it is used for nothing else. That is a stronger, more citable position than the EU's.

---

## 2. GDPR: lawful basis, security obligations, DPIA, Art 22

### 2.1 Lawful basis — Art 6(1)(f) legitimate interests is the right basis; Art 6(1)(d) vital interests is wrong; consent is a fallback

- **Recital 47** — "[t]he processing of personal data strictly necessary for the purposes of preventing fraud ... constitutes a legitimate interest of the data controller concerned."
- **Recital 49** — network and information security processing "to the extent strictly necessary and proportionate" is a legitimate interest.
- **EDPB Guidelines 1/2024 on processing of personal data based on Article 6(1)(f)**, v1.0 adopted 8 October 2024 (consultation 9 Oct – 20 Nov 2024). **Important currency note: as of at least 15 September 2026, only v1.0 exists; there is no final version yet.** `[verified via a secondary tracker; the EDPB document page for the final version returned 404 when I fetched it, consistent with "no final version".]`

  What the draft says that matters to us:
  - ¶126: "Measures to ensure an appropriate level of network and information security may entail processing of personal data. Such processing activities **may, in principle, be based on Article 6(1)(f) GDPR**, provided that its conditions (including the necessity and balancing tests) are complied with. This was acknowledged – although indirectly – by the CJEU in _Breyer_, as well as in Recital 49 GDPR, and in Recital 121 of Directive (EU) 2022/2555."
  - ¶127: "the collection and analysis of personal data for the purposes of ensuring a high level of network and information security **must meet both the necessity and balancing tests**. This implies that the objective of security cannot justify an excessive processing of personal data." The EDPB warns about "intrusive analysis of communication content and meta data, which may have a significant impact on the outcome of the balancing test," and cites _Meta v Bundeskartellamt_ for the proposition that you must verify whether "the legitimate interest pursued cannot reasonably be achieved just as effectively by other means less restrictive" and whether Art 5(1)(c) has been observed.
  - ¶29: the "strict necessity" requirement; "it is generally easier for a controller to demonstrate the necessity of the processing to pursue its own legitimate interests than to pursue the interests of a third party, and ... the latter kind of processing is generally less expected by the data subjects." → **Use your own server-side fingerprint store, not a vendor's network, if you can.**
  - The three-step test (¶¶24–38): legitimate interest → necessity → balancing, with the balancing inputs being (i) data subjects' interests/rights, (ii) impact (nature of data, context, further consequences), (iii) **reasonable expectations**, (iv) final balancing incl. mitigating measures.
  - It also has a dedicated chapter on **children's data** and on **preventing fraud** — read both before finalising.
  - `[I extracted and read the fraud/NIS/children's chapter headings and the NIS paragraphs verbatim; I did not read every paragraph of the 37-page document.]`

- **Art 6(1)(d) vital interests** — "in order to protect the vital interests of the data subject or of another natural person". This is for life-threatening emergencies. It is not a basis for routine abuse prevention. **Do not use it.** Mentioning it invites a DPA to conclude you did not understand your own processing.
- **Art 32 (security of processing)** — this is an _obligation_, not a legal basis. It requires appropriate technical and organisational measures, "including inter alia as appropriate: (a) the pseudonymisation and encryption of personal data; (b) the ability to ensure the ongoing confidentiality, integrity, availability and resilience of processing systems and services; ...". Useful because it makes some security logging mandatory-adjacent, which strengthens the "strictly necessary" argument.
- **Art 25 (data protection by design and by default)** — requires technical implementation of data-minimisation principles, and Art 25(2) requires default settings that limit the amount of personal data processed. This is the article that makes "collect the minimum at collection time" a legal obligation, not a nicety.
- **Art 5(1)(c) minimality** and **Art 5(1)(e) storage limitation** — see §3.

### 2.2 Art 22 — is an automatic ban an "automated individual decision"?

Art 22(1): a data subject has the right not to be subject to a decision **based solely on automated processing** which produces **legal effects** concerning them or **similarly significantly affects** them. If it applies, you need Art 22(2) gateways (contract necessity, law, or explicit consent) and Art 22(3) safeguards: at minimum the right to **obtain human intervention**, to **express a point of view**, and to **contest the decision**.

The EDPB-endorsed text on this remains **WP29 Guidelines on Automated individual decision-making and Profiling for the purposes of Regulation 2016/679 (WP251rev.01)**, endorsed by the EDPB. `[I did not fetch WP251 in this session; cite it as the reference text.]`

**The decisive enforcement precedent is now UBER.** Dutch DPA (Autoriteit Persoonsgegevens), announced 24 August 2026 via CNIL, decision published by the EDPB 8 October 2026:

- Fine: **€824,990,000** on UBER B.V. and UBER TECHNOLOGIES INC.
- Legal references: **Article 22** and **Article 13**.
- Facts: "les désactivations des comptes des chauffeurs **en cas de suspicion de fraude** (désactivation temporaire) ainsi qu'**en cas de notation basse de la part des clients** (désactivation temporaire et définitive) constituent des décisions individuelles automatisées **en raison de l'absence totale d'intervention humaine dans le processus de décision**." Drivers could not work and lost income.
- Context: complaints from 171 French drivers via Ligue des droits de l'Homme to CNIL; Dutch AP led under the one-stop-shop; incidents 2018–2022. Prior fines in the same case family: **€10M (11 Dec 2023, information)** and **€290M (22 July 2024, transfers)**.
- **Under appeal; no final judicial decision.** `[verified: CNIL page + EDPB national news page; AP press release URL is on the CNIL page.]`

**Reading for our design** `[analysis]`:

- A ban on a free, non-commercial Quran reader is probably **not** "similarly significant" in the WP251 sense (no credit, employment, housing, essential service denial). So Art 22 may not formally apply.
- **But** the Uber case shows the pattern that regulators punish: _fully automated, unexplained, irreversible, income/service-affecting decisions with zero human touchpoint_. And the app is a religious service — denial of access to religious content is a different kind of harm than denial of a shopping account, and a DPA could take that seriously.
- **Design rule: never make permanent bans fully automatic.** Human review before any permanent ban; a stated reason; an appeal channel. This costs almost nothing and removes the entire Art 22/Art 13/Uber class of exposure. See §7.

### 2.3 DPIA (Art 35)

- Art 35(1): DPIA required where a type of processing is "likely to result in a high risk to the rights and freedoms of natural persons", in particular using new technologies, and taking into account nature/scope/context/purposes.
- Art 35(3) mandatory triggers: systematic and extensive **evaluation of personal aspects based on automated processing, including profiling**, and decisions with legal/similarly significant effects; **large scale** special-category or criminal-conviction data; **systematic monitoring of a publicly accessible area on a large scale**.
- The reference methodology is **WP248 rev.01 (2017)**, which lists **nine criteria**; the rule of thumb is that **two criteria met → DPIA required**. The criteria include: evaluation/scoring; automated decision-making with legal or similarly significant effect; systematic monitoring; sensitive data or data of a highly personal nature; data processed on a large scale; matching or combining datasets; data concerning vulnerable subjects (incl. children); innovative use of technology; preventing data subjects from exercising a right or using a service.
- `[I did not fetch WP248 in this session — it is a stable, well-known document; verify the criterion wording before quoting.]`

**Applying it to a small Quran app** `[analysis]`:

- Criteria likely met: **sensitive data / highly personal nature** (religious belief, §6.7); arguably **innovative use of technology** (fingerprinting) and **preventing data subjects from using a service** (ban enforcement). Possibly **vulnerable subjects** (minors).
- Criteria probably not met: large scale (no); systematic monitoring of a publicly accessible area (no — an online service is not an "area"); evaluation/scoring with significant effect (no).
- **Verdict: it is defensible either way, but if two criteria are genuinely met you should do a DPIA.** A short, honest DPIA (2–4 pages) is cheap and is the single best artefact to show a DPA you did the balancing test for Art 6(1)(f) — the EDPB's own LI guidelines treat the balancing exercise as a documentation duty. EDPB published a **DPIA template** for consultation (14 April – 9 June 2026); use it if it fits.
- Also check your lead Member State's **Art 35(4) national list** of processing requiring a DPIA. `[not checked per Member State.]`

---

## 3. Retention: how long may a device identifier be kept for security?

### 3.1 There is no EU-wide number. The obligation is a process, not a period.

Art 5(1)(e) requires storage "no longer than is necessary for the purposes for which the personal data are processed". CNIL's own page on retention (updated **2 April 2026**) is explicit that in most cases **the duration is not fixed by any text and "il appartient alors au responsable du fichier de la déterminer en fonction de la finalité du traitement"**. CNIL's lifecycle model: **base active** (needed to achieve the purpose) → **archivage intermédiaire** (closed files, legal-evidence purposes, access restricted to specifically authorised staff, physically or logically separated from the active base) → **archivage définitif** (rare).

Critically, CNIL says its sectoral referentials are **"soit obligatoires ... soit recommandées"**, and recommended durations "constituent alors **un point de repère** dont le responsable de traitement peut s'éloigner **sous réserve de documenter son choix**. Le fait d'appliquer ces durées recommandées constitue une **présomption de conformité**."

**There is no CNIL referential for "security fingerprinting".** The published referentials cover health (excl. research), health research, social/medico-social, and HR (April 2026). So you are in "determine it yourself and document it" territory.

### 3.2 Numbers that DPAs and other regulators actually treat as normal

These are the anchors I could verify. They are _not_ a safe harbour for fingerprinting; they are the range in which a security retention period will look unremarkable.

| Source                                                     | Period                                                                                                               | What it covers                                                                                                                                                                                                                              |
| ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **India, DPDP Rules 2025, Rule 8(3)**                      | **minimum 1 year**                                                                                                   | Personal data + "associated traffic data and other logs of the processing", retained for the purposes in the Seventh Schedule, then erased unless other law requires more. Applies to a Data Fiduciary generally, not only large platforms. |
| **India, DPDP Rules 2025, Third Schedule (Rule 8(1))**     | **3 years of inactivity**                                                                                            | Deemed end of purpose for large e-commerce/gaming/social platforms (≥2 crore / ≥50 lakh registered users). Not applicable to a small app, but shows 3 years as the outer "dormancy" norm.                                                   |
| **Fingerprint.com (vendor)**                               | **30 days** typical; **90 days** for Enterprise; longer only by contract; webhooks for self-hosting                  | "Visit history" availability for the visitor ID.                                                                                                                                                                                            |
| **France, travel-documents decree (public sector, fraud)** | **3 years** from the fraudulent application                                                                          | Example of a 3-year fraud-data norm in French law. `[verified via Legifrance snippet, not the full text.]`                                                                                                                                  |
| **ICO (statistical exception)**                            | not longer than needed for aggregation; CNIL/ICO suggest daily aggregation as a reasonable default                   | Individual-level analytics logs.                                                                                                                                                                                                            |
| **ICO (strictly necessary / security)**                    | no period stated; "you **must** limit the use of the storage and access technology to the purpose that is necessary" | Security technologies.                                                                                                                                                                                                                      |

### 3.3 What DPAs have fined for over-retention

`[Honest limitation: I did not open individual over-retention decisions in this session. The following is the shape of the enforcement landscape, with sources to check.]`

- CNIL's sanctions register is at `cnil.fr/fr/thematique/cnil/sanctions` and `cnil.fr/fr/les-sanctions-prononcees-par-la-cnil`. Its 2025 review names cookies, employee surveillance and data security as the main themes — retention violations typically appear as a _secondary_ finding inside a security or transparency case, not as a standalone headline fine.
- The **Uber** decision (€824.99M, §2.2) is the closest analogue to what a ban system does, and it was decided under Art 22 + Art 13, not retention.
- The **Criteo** CNIL decision (June 2023, €40M) is the best-known adtech case combining consent, retention and DSAR failures; I cite it here as the pattern to avoid (keep identifiers, fail to delete on withdrawal) rather than as a verified retention precedent. `[not verified in this session.]`

### 3.4 The trade-off you must acknowledge in writing

Longer retention helps ban evasion detection (the abuser returns on the same device in 6 months and you recognise them). Shorter retention is what minimises the Art 5(1)(e) and Art 9 exposure. There is no free lunch, and no DPA has published a fingerprint-specific safe harbour.

**A defensible ladder** `[analysis — this is the recommendation, not a legal rule]`:

1. **Raw fingerprint / signal bundle** (the actual browser attributes, TLS/JA4 hash, header set): **do not persist at all.** Compute, compare, discard. Store only the derived key.
2. **Derived device key for the session/auth** (peppered HMAC, see §8): **session lifetime + 30 days** for "remember this device" purposes, matching vendor norms.
3. **Ban/abuse record** (device key + reason code + timestamp + who decided): **12 months**, then either delete or reduce to a **count-only** record ("this device was banned once, reason class X") with no linkage to a user account. 12 months is the shortest period that still catches seasonal/repeat abuse.
4. **Aggregate counters** (no identifier): keep indefinitely — this is not personal data if genuinely non-linkable.
5. **Every record carries an expiry column**, and a scheduled job enforces it (§8).

If you want 3 years (the DPDP Third Schedule / French fraud norm), you need a written justification tied to observed repeat-abuse intervals. Do not pick 3 years by default.

---

## 4. Consent UI: if consent is required, what is compliant — and can it be separate from the cookie banner?

### 4.1 When you actually need consent

Under EU ePrivacy you need consent if (a) you cannot bring the fingerprint within "strictly necessary for the ISS explicitly requested by the user", or (b) you use it for **any second purpose**. Under UK PECR Sch A1 ¶4, the same rule applies — and the ICO states it bluntly: use it for another purpose and "the exception does not apply".

Concretely, these uses **break** the security exception and force consent (ICO's own table):

- cross-site or cross-device tracking;
- anything advertising-adjacent, "including (but not limited to) things such as frequency capping, ad affiliation, ad measurement and performance, click fraud detection, market research, product improvement or debugging";
- using the fingerprint for analytics, "A/B testing", personalisation, or "content personalisation or usage monitoring".

That last one is a trap for a small app: a single analytics event that keys off the device identifier converts the whole thing into a consent-requiring technology.

### 4.2 What valid consent looks like (EU/UK)

GDPR Art 4(11) + Art 7: freely given, specific, informed, unambiguous, by statement or clear affirmative action; Art 7(3) withdrawal as easy as giving; Art 7(1) the controller bears the burden of proof. Art 7(2) requires the consent request to be clearly distinguishable if bundled. Planet49: pre-ticked boxes are not valid consent. EDPB Cookie Banner Taskforce: reject must be as prominent and as easy as accept; no misleading colour/contrast; no cookie walls.

### 4.3 Can it be a separate, non-cookie-banner flow? Yes — and that is usually the better design.

Nothing in Art 5(3) or PECR requires a _banner_. What is required is **clear and comprehensive information about the purpose** + a **freely given specific consent** before the storage/access happens. Practical patterns that regulators accept:

- **Just-in-time consent at the point of use.** The ICO explicitly endorses this pattern for embedded content: "use a 'just-in-time' approach to seek consent on particular pages where the videos are included." The same logic applies to a fingerprint that is only used on the login and report-submission pages.
- **Trigger-on-demand (vendor pattern).** Fingerprint.com's own documentation: "Instead of identifying visitors automatically on page load, you can choose to explicitly trigger the Fingerprint identification request only in specific circumstances. For example, only after the visitor grants consent by clicking 'Accept' on a cookie consent banner." `[verified: docs.fingerprint.com/docs/privacy-and-compliance]`
- **Browser-signal consent.** In the UK only, PECR Sch A1 ¶2(3) says consent may be signified by "amending or setting controls on the internet browser". The ICO warns you "**must not** solely rely on browser settings as an indication of whether a person does not object" — so treat this as an additional route, not your primary one.

**What a compliant security-consent flow looks like** `[analysis]`:

1. Trigger only on the pages/actions that need it (login, signup, abuse-report, ban-appeal), never on page load of the reader.
2. A short, plain-language, purpose-specific panel — not the cookie banner — with the actual purpose named ("to detect abuse and prevent ban evasion"), what is computed, where it is stored, how long, and the fact that it is **not** used for analytics or advertising.
3. **Accept** and **Decline** with equal visual weight, both one click. Decline must not degrade the service beyond losing the convenience feature (never block the reader).
4. A working, one-click withdrawal in account settings, and a documented process to delete the derived key on withdrawal.
5. Log the consent event (timestamp, version of the text, what was consented to) — Art 7(1) burden of proof.
6. Do not re-ask for a broader consent later; if the purpose changes, that is a new consent.

### 4.4 The "fingerprint login" feature — this is where Art 5(3) gets easy

If the user _asks_ for "remember this device / sign me in without an email code", the fingerprint is part of a service the user explicitly requested, which is exactly the Art 5(3) text ("strictly necessary in order for the provider of an information society service explicitly requested by the subscriber or user to provide the service"). Both DPAs have said so:

- ICO SAT guidance, strictly-necessary table: "**Identifying a user once they have logged in to an online service for the duration of their visit to the site** (eg to prevent a new login prompt on an online banking service each time the user loads a new page)." ✔
- PECR Sch A1 ¶4(2)(e)(i): "**automatically authenticating the identity of the subscriber or user**".

So a fingerprint-based "trusted device" login is the _easiest_ thing to defend. Risks to manage:

- **Purpose creep.** The moment the login fingerprint and the abuse-ban fingerprint are the same key, you have one identifier serving two purposes, and the strict-necessity argument for each weakens. Either use the same key and document it as one purpose ("account security"), or keep two keys.
- **Credential semantics.** If the fingerprint becomes a login credential, Art 32 applies with full force: it must be resistant to replay/spoofing, rate-limited, and never a single factor. A spoofable fingerprint used as an authentication factor is a security vulnerability you have now documented in your privacy policy.
- **Art 5(1)(c) minimality.** If you can achieve "remember this device" with a plain random cookie/token in httpOnly storage (which you can), that is _less_ intrusive than a fingerprint and the EDPB's necessity test prefers it. Consider: use a random token for "remember me", and reserve the fingerprint for abuse signals only. `[analysis — this is the cleanest design and I recommend it.]`
- **Note the EDPB's unique-identifier discussion** (Guidelines 2/2023 ¶¶61–63): hashing persistent personal data on the user's device to create a unique identifier still triggers Art 5(3), because the terminal equipment is instructed to send it. Hashing does not create an exemption; it only affects the data-protection analysis.

---

## 5. United States

### 5.1 CCPA/CPRA: is a device fingerprint "sensitive personal information"?

**No — a device identifier is ordinary "personal information"; precise geolocation and religious belief are SPI.**

Verified from Cal. Civ. Code §1798.140 (source: leginfo, Jan 1 2026 version, read via california.public.law mirror):

- **§1798.140(aj) "Unique identifier" / "unique personal identifier"** = "a persistent identifier that can be used to recognize a consumer, a family, or a device that is linked to a consumer or family, over time and across different services, including, but not limited to, **a device identifier; an Internet Protocol address; cookies, beacons, pixel tags, mobile ad identifiers, or similar technology**; customer number, unique pseudonym, or user alias; telephone numbers, or **other forms of persistent or probabilistic identifiers** that can be used to identify a particular consumer or device". → A browser fingerprint is squarely inside this definition. It is PI, not SPI.
- **§1798.140(ae) "Sensitive personal information"** includes:
  - (1)(C) "**A consumer's precise geolocation.**"
  - (1)(D) "A consumer's racial or ethnic origin, citizenship or immigration status, **religious or philosophical beliefs**, or union membership." ← **directly relevant to a Quran app.**
  - (2)(A) "The processing of biometric information for the purpose of uniquely identifying a consumer."
- **§1798.140(w) "Precise geolocation"** = "any data that is derived from a device and that is used or intended to be used to locate a consumer within a geographic area that is equal to or less than the area of a circle with a **radius of 1,850 feet**, except as prescribed by regulations."
- **§1798.140(c) "Biometric information"** = "an individual's physiological, biological, or behavioral characteristics, including ... DNA, that is used or is intended to be used ... to establish individual identity. Biometric information includes ... imagery of the iris, retina, fingerprint, face, hand, palm, vein patterns, and voice recordings, from which an identifier template ... can be extracted, and **keystroke patterns or rhythms, gait patterns or rhythms**, and sleep, health, or exercise data that contain identifying information." `[analysis: a browser/device fingerprint is not a person's physiological/biological/behavioural characteristic used to establish identity — it is a device attribute set. Standard reading is that CCPA "biometric information" does not cover device fingerprinting. This is untested; do not rely on it as a shield if you ever add real biometrics.]`

**Business thresholds (§1798.140(d)(1)):** >$25M annual gross revenue (CPI-adjusted), **or** buys/sells/shares PI of ≥100,000 consumers or households, **or** ≥50% of revenue from selling/sharing PI. **A small non-commercial app with no selling/sharing is almost certainly not a "business" under CCPA at all.** `[analysis — but note the "in combination" wording; if you ever monetise or share data with a vendor for the vendor's purposes, re-run this.]`

### 5.2 Security/fraud carve-outs in CCPA

- **§1798.140(ac) "Security and integrity"** = the ability of "(1) Networks or information systems to detect security incidents that compromise the availability, authenticity, integrity, and confidentiality of stored or transmitted personal information. (2) Businesses to **detect security incidents, resist malicious, deceptive, fraudulent, or illegal actions and to help prosecute those responsible for those actions**. (3) Businesses to ensure the physical safety of natural persons."
- **§1798.140(e)(2)** makes "Helping to ensure security and integrity to the extent the use of the consumer's personal information is reasonably necessary and proportionate for these purposes" a **"business purpose"** — which is what lets you use PI without it counting as a "sale"/"share" and lets service providers act on your behalf.
- **§1798.145(a)** exemptions (obligations do not restrict ability to): comply with law; comply with civil/criminal/regulatory inquiry; cooperate with law enforcement; emergency access to a consumer's PI; exercise or defend legal claims; collect/use/retain/sell/share **deidentified or aggregate** information; wholly-out-of-California conduct. **There is no blanket "fraud prevention" exemption** in §1798.145 — the security/fraud justification lives in the "business purpose" + "security and integrity" definitions and in the regulations.
- **CPPA regs §7002** (Restrictions on Collection and Use): collection/use/retention/sharing "shall be **reasonably necessary and proportionate** to achieve" the disclosed purpose, and the purpose "shall be **consistent with the reasonable expectations** of the consumer(s)". Example in the reg text: "the consumer of a business's mobile flashlight application would not expect the business to collect the consumer's geolocation information to provide the flashlight service."
- **CPPA regs §7050(a)(4)**: a service provider/contractor may retain/use/disclose PI "**to prevent, detect, or investigate data security incidents or protect against malicious, deceptive, fraudulent or illegal activity**, even if this business purpose is not specified in the written contract".

### 5.3 Global Privacy Control — must you honour it?

**CPPA regs §7025(b), verbatim:** "**A business that sells or shares personal information shall process any opt-out preference signal** that meets the following requirements as a valid request to opt-out of sale/sharing: (1) The signal shall be in a format commonly used and recognized by businesses. An example would be an HTTP header field or JavaScript object. ..." §7025(c)(1): treat it as a valid opt-out for that browser/device and any associated profile, "including pseudonymous profiles."

**So: GPC is a sale/sharing opt-out mechanism.** If your fingerprint is used **only** for security, and you do not sell or share PI (including not letting a fingerprint vendor use your data for its own purposes), **§7025 is not triggered and you do not need to honour GPC for the security fingerprint.** `[analysis, grounded in the reg text I read.]`

**But this is the trap:** if you use a third-party fingerprint vendor whose network pools your users' data to serve other customers, or whose terms let it use the data for its own product improvement in a way that is not your "business purpose", that is at minimum a **third-party disclosure** and plausibly a "share" — at which point the GPC/opt-out machinery, the "Do Not Sell or Share" link, and the §7051 contract requirements all attach. **Mitigation:** vendor as a **service provider** under a written contract with the §7051 restrictions, no cross-customer data pooling, no vendor-side model training on your data. (Fingerprint.com's docs claim exactly this posture: "The visitor ID is scoped to each Fingerprint workspace, not the internet at large. If two Fingerprint customers identify the same browser or device, they will receive different visitor IDs. This intentionally makes Fingerprint unsuitable for creating a global visitor activity graph across _unrelated_ sites" — and they act as a "Service Provider" under a US SPL DPA. `[verified: docs.fingerprint.com]`)

**Also note the CPPA is actively revisiting opt-out preference signals** — "Opt–out Preference Signals (OOPS)" was an open preliminary-rulemaking topic whose comment period closed 6 April 2026. `[verified on cppa.ca.gov/regulations]` So the §7025 position may move.

### 5.4 Risk assessments, cyber audits and ADMT under the new CPPA regs (effective 1 Jan 2026)

The "CCPA Updates, Cybersecurity Audits, Risk Assessments, and ADMT" package was **adopted 24 July 2025, approved by OAL 22 September 2025, effective 1 January 2026**. `[verified: cppa.ca.gov/regulations/ccpa_updates.html]` Relevant thresholds, read from the approved regulation text:

- **§7120 cybersecurity audits:** required only if (1) the business meets §1798.140(d)(1)(C) (>50% revenue from selling/sharing), **or** (2) it meets (d)(1)(A) (>$25M) **and** processed PI of ≥250,000 consumers/households, **or** SPI of ≥50,000 consumers. First audit reports due April 2028 / 2029 / 2030 by revenue band. → **A small app is out of scope.**
- **§7150 risk assessments — the triggers:** (1) selling or sharing PI; (2) **processing sensitive personal information**; (3) using ADMT for a significant decision; (4) automated inference about intelligence/ability/aptitude/performance at work/economic situation/health/preferences/interests/reliability/predispositions/behaviour/location/movements **based on systematic observation while acting as an education applicant, job applicant, student, employee or contractor**; (5) same inferences **based on presence in a sensitive location**; (6) processing PI to train ADMT for a significant decision or identity-verifying/facial/emotion-recognition tech.
  → **The only trigger a Quran app could plausibly hit is (2), if user data is treated as revealing religious belief (SPI).** That is a real risk, and it is the same risk as §6.7. If you conclude your processing does not "reveal" religious belief to you (because you never infer or use it), you can document that and rely on it — but write the analysis.
- **§7200 ADMT:** applies to "a business that uses ADMT to make a **significant decision** concerning a consumer." **§7001 defines "significant decision" narrowly**: provision/denial of "financial or lending services, housing, education enrollment or opportunities, employment or independent contracting opportunities or compensation, or healthcare services." → **An account ban on a Quran app is not a "significant decision" under the CPPA regs**, so the ADMT notice/opt-out/access obligations (§§7220–7222) do **not** attach. This is a meaningful, checkable conclusion. `[verified from the approved reg text.]`

### 5.5 California Delete Act / DROP

**DROP (Delete Request and Opt-out Platform) is live.** Timeline from the CPPA's own DROP site: **1 January 2026** — DROP launches and Californians can submit requests; **1 August 2026** — data brokers must access DROP at least once every 45 days and begin processing deletion requests; **November 2026** — all registered data brokers should have completed their first download/upload cycle and must delete data every 45 days. Over 600 registered data brokers are in scope.

**Applicability:** DROP is a **Delete Act** mechanism aimed at **data brokers** — businesses that sell/share PI about consumers **with whom they do not have a direct relationship**. `[analysis: a Quran app with user accounts has a direct relationship with its users and does not sell/share PI, so it is not a data broker and DROP does not apply. Confirm against the Delete Act's definition if you ever add a free-tier product with no account.]`

### 5.6 State laws (Texas, Oregon) and biometric statutes

`[Caution: I did not fetch the Texas or Oregon statute texts in this session. The following is orientation, flagged unverified.]`

- **Texas Data Privacy and Security Act** — effective **1 July 2024**; a small-business exemption applies but small businesses may not sell sensitive data without consent; sensitive-data processing generally requires consent. Texas amended its framework in 2025 (SB 261) including minors' provisions; the compliance dates are worth re-checking.
- **Texas CUBI** (Tex. Bus. & Com. Code §503.001) regulates **biometric identifiers** (retina/iris scan, fingerprint, voiceprint, record of hand or face geometry). A **browser** fingerprint is not a biometric identifier. Note the naming collision: "fingerprint login" via WebAuthn/passkeys does **not** give you the biometric — the biometric stays on the device and you receive a public-key assertion. That is a real compliance advantage; do not break it by trying to collect biometric templates.
- **Oregon Consumer Privacy Act** — effective **1 July 2024**. Oregon has been one of the more prescriptive state laws (opt-in consent for sensitive data). Verify the current text and any 2025–2026 amendments before relying on it.
- Other biometric statutes worth a line in the design doc: **Illinois BIPA** (private right of action, per-violation damages), **Washington HB 1493**, **Colorado**, **New York City**. All of them key off _biometric identifiers of a person_, not device fingerprints. Again: keep the biometrics on-device.

### 5.7 FTC and biometric/fingerprint data

- **FTC v. Rite Aid** (Dec 2023): five-year prohibition on using facial-recognition technology for security or surveillance purposes, plus deletion of photos/videos, over the retailer's failure to implement reasonable procedures — false accusations, disproportionate misidentification of people of colour, women and younger shoppers. Press release: `ftc.gov/news-events/news/press-releases/2023/12/rite-aid-banned-using-ai-facial-recognition-after-ftc-says-retailer-deployed-technology-without`; case page `ftc.gov/legal-library/browse/cases-proceedings/2023190-rite-aid-corporation-ftc-v`. `[verified URL]`
- **In re Everalbum, Inc.** (May 2021): FTC action over the "Ever" photo app's facial-recognition feature; the order required deletion of the **models/algorithms** derived from user photos, not just the photos. This is the canonical "you cannot keep the derived artefact after you lose the data" case. `[I could not verify the press-release URL in this session — my guessed URLs 404'd. Cite the FTC legal library entry rather than a specific press-release link until verified.]`
- **The generalisable FTC principle** `[analysis]`: Section 5 unfairness/deception turns on (a) what you told users, and (b) whether your security tech is accurate enough that it doesn't systematically harm people. Rite Aid's sin was _inaccuracy plus surveillance_, not fingerprinting. If you build abuse scoring, **measure false-positive rates across device classes and be able to show the numbers.**
- **COPPA Rule amendments** (see §6.8) explicitly add **biometric identifiers** and government-issued identifiers to the definition of personal information, and cap retention at "as long as reasonably necessary".
- Also relevant: FTC actions on **sensitive location data** (X-Mode/Outlogic, InMarket, Kochava litigation) — the through-line is that precise location plus device identifiers is the FTC's current sensitive-data priority. **Do not collect precise geolocation for a Quran app.** If you need coarse geolocation for abuse triage, keep it at country level and never persist it with the device key.

---

## 6. Other regimes, and the two extra duties that actually matter

### 6.1 Brazil (LGPD) — has a _security-specific_ basis, and it is unusually useful

Read directly from Lei 13.709/2018 (Planalto consolidated text; note it has been amended up to **Lei 15.352/2026**, which changed the authority's status to an agency).

- **Art 5(II) sensitive personal data** includes "**convicção religiosa**" (religious conviction), political opinion, union or religious/philosophical/political organisation membership, health, sexual life, **genetic or biometric data**. → A Quran app's user data is sensitive-adjacent in Brazil too.
- **Art 7(IX)** legitimate interests — available for ordinary personal data, "exceto no caso de prevalecerem direitos e liberdades fundamentais do titular".
- **Art 10** — legitimate interest requires: only data "**estritamente necessários**" for the purpose (Art 10 §1); transparency measures (Art 10 §2); and the ANPD may demand a **DPIA** ("relatório de impacto à proteção de dados pessoais") when the basis is legitimate interest (Art 10 §3).
- **Art 11(II)(g)** — **the security basis**: sensitive data may be processed **without consent** where indispensable for "**garantia da prevenção à fraude e à segurança do titular, nos processos de identificação e autenticação de cadastro em sistemas eletrônicos**, resguardados os direitos mencionados no art. 9º ... e exceto no caso de prevalecerem direitos e liberdades fundamentais do titular". → This is a statutory security/authentication carve-out for sensitive data. It is the cleanest fit I found anywhere for "fingerprint used for account identification/authentication and fraud prevention" on data that reveals religious belief.
- **Art 11(1)** — "Aplica-se o disposto neste artigo a **qualquer tratamento de dados pessoais que revele dados pessoais sensíveis** e que possa causar dano ao titular" → the sensitive-data regime applies to _anything that reveals_ sensitive data. Same broad "reveal" logic as GDPR Art 9 / CJEU C-184/20.
- **Art 12** — anonymised data is not personal data unless the process is reversible using own means or reasonable efforts; **Art 12 §2** — data used to form a behavioural profile of an identified person can still be personal data.
- **Art 13(4)** — defines pseudonymisation: data that loses direct/indirect association except by use of additional information kept separately by the controller in a controlled, secure environment. (Useful: Brazil has an explicit statutory definition matching the design in §8.)
- **Art 14** — children's and adolescents' data: best interests; **specific and highlighted parental consent** for children (Art 14 §1); controllers must make public what they collect and how rights are exercised (§2); limited no-consent collection only to contact the parent, used once and not stored, or for the child's protection (§3); must not condition participation on providing more data than strictly necessary (§4); **"realizar todos os esforços razoáveis"** to verify the consent was given by the responsible adult (§5).
- **Art 15/16** — termination of processing and deletion, with narrow retention exceptions (legal obligation, research, transfer, anonymised exclusive use).
- **Art 20** — right to request **review of decisions taken solely on the basis of automated processing** that affect the data subject's interests, including profiling; Art 20(1) the controller must provide clear and adequate information about the criteria and procedures, subject to trade secrets; Art 20(2) the ANPD may audit for discriminatory aspects. → **Brazil gives a review right that is broader than GDPR Art 22** because Art 20 does not require "legal or similarly significant effect". A ban imposed by automation is likely within Art 20. `[analysis]`

### 6.2 UK — Data Protection Act 2018 + UK GDPR + PECR

Covered in §1.6. Additional UK-specific items that matter:

- **DUAA s.80 (Automated decision-making)** amended the UK GDPR's Art 22 regime, and **Schedule 4** introduced **"recognised legitimate interests"** — a list of purposes where the balancing test is not required. `[I did not read the Schedule 4 list in this session; verify whether crime prevention/security appears there, because if it does, it materially strengthens the UK legitimate-interest position.]`
- **DUAA s.81** inserts a "data protection by design: **children's higher protection matters**" obligation into the UK GDPR — i.e. children's interests must be considered in design. Relevant to §6.8.
- **ICO "Profiling tools for online safety"** guidance — linked from the SAT guidance's strictly-necessary chapter under "Further reading". Read this before building abuse scoring.
- **Age Appropriate Design Code (AADC)** — 15 standards for ISS likely to be accessed by children. If the app is a religious reader with no accounts for under-13s, exposure is limited; if minors can create accounts, the AADC is in play (data minimisation, transparency, no profiling of children, no nudge techniques).

### 6.3 India (DPDP Act 2023 + DPDP Rules 2025) — the biggest design constraint

**The Rules were notified 13 November 2025 (G.S.R. 846(E)), 23 rules + 7 schedules, and commence in three phases:**

| Date            | What comes into force                                                             |
| --------------- | --------------------------------------------------------------------------------- |
| **13 Nov 2025** | Rules 1, 2 and 17–21 (Board establishment, appointments, digital office, appeals) |
| **13 Nov 2026** | Rule 4 (Consent Managers)                                                         |
| **13 May 2027** | **Rules 3, 5–16, 22 and 23** — i.e. everything substantive                        |

`[verified via the Rules' commencement provision as reproduced by a bare-acts mirror, cross-checked against a law-firm timeline and a second mirror. The phase table is consistent across three sources.]`

Substantive rules that matter to this design:

- **Rule 3 (Notice):** a standalone notice, clear and plain language, with an **itemised description** of the personal data and the specified purposes, and how to withdraw consent and exercise rights.
- **Rule 6 (Reasonable security safeguards)** and **Rule 7 (Breach intimation):** intimation to affected Data Principals without delay, and a detailed report to the Board **within 72 hours** of becoming aware.
- **Rule 8 (Retention)** — three separate mechanisms, all worth reading in full:
  - Rule 8(1) + **Third Schedule**: for large e-commerce (≥2 crore registered users), gaming (≥50 lakh) and social-media (≥2 crore) fiduciaries, the specified purpose is deemed no longer served after **3 years** of the Data Principal not approaching the fiduciary or exercising rights, and data must be erased. Account-access and virtual-token purposes are carved out.
  - Rule 8(2): at least **48 hours' warning** before that erasure, unless the user logs in / contacts / exercises rights.
  - **Rule 8(3): "a Data Fiduciary shall retain ... such personal data, associated traffic data and other logs of the processing for a minimum period of one year from the date of such processing"** for the prescribed purposes, then erase unless other law requires more. This applies to a Data Fiduciary generally, not only the large classes.
- **Rule 10 (Verifiable parental consent for a child)** + **Rule 11 / Fourth Schedule (exemptions)**. "Child" means **under 18**; "adult" means 18+. Verification routes: reliable identity/age details already held; or details voluntarily provided; or a **virtual token** mapped to those details issued by an authorised entity (including a Digital Locker service provider). Rule 10's illustrations show both child-initiated and parent-initiated registration flows. The commentary stresses: verification ≠ consent; the parent must still get notice and give specific, informed, unbundled consent; pre-consent processing must be limited to what is needed to run the verification flow.
- **Section 9(3) of the Act** — a Data Fiduciary must **not undertake tracking or behavioural monitoring of children, or targeted advertising directed at children**. The Fourth Schedule exemptions from s.9(1) and s.9(3) cover, among others, healthcare, **educational institutions** (monitoring for educational activities or the safety of enrolled children), crèches/day-care, and school-transport location tracking. **There is no "security/anti-abuse" exemption in the Fourth Schedule.** `[analysis: this is a gap. A security fingerprint applied to a child's session could be argued to be neither "tracking" nor "behavioural monitoring" in the intended sense, but that reading is untested and there is no exemption to lean on.]`
- **Rule 13 (Significant Data Fiduciary):** DPIA plus audit once every 12 months, plus due diligence on algorithmic software. SDF status depends on factors including volume and sensitivity of data and risk to rights — not on a bright-line threshold. `[analysis: a small Quran app should not be an SDF, but "sensitivity of data" is a listed factor and the app's data is belief-revealing. Document why you are not an SDF.]`
- **Penalties** up to **₹250 crore** per instance for the most serious breaches. `[from the Rules summary / PIB materials; verify the Schedule of penalties in the Act before quoting.]`

**The key India design consequence** `[analysis]`: **DPDP has no legitimate-interest basis and no ePrivacy-style "strictly necessary" carve-out.** Processing rests on consent (s.6) or one of the s.7 "certain legitimate uses". I did not verify whether fraud prevention appears in s.7. If it does not, then for Indian users:

- the security fingerprint is consent-based, **or**
- it must be processed in a form that is genuinely not "digital personal data" (i.e. cannot identify the Data Principal). DPDP applies to _digital personal data_ — data about an identifiable individual. A truly non-linkable abuse counter may fall outside the Act. `[analysis; get this checked.]`
- and for under-18s, Rule 10 + s.9(3) apply, which is a hard constraint.

### 6.4 Japan (APPI)

- The APPI's 2020/2022 amendments introduced **pseudonymised information** (仮名加工情報) and **anonymously processed information** regimes, with PPC guidelines at `ppc.go.jp/personalinfo/legal/guidelines_anonymous/` (the guidelines covering both anonymous and pseudonymised processing). Pseudonymised information permits internal use under relaxed rules, with a prohibition on re-identification and restrictions on third-party provision. `[URL verified via search; I did not read the Japanese text.]`
- PPC's English portal: `ppc.go.jp/en/`.
- **The APPI is mid-reform.** The PPC's 2024 Interim Summary → 2025 consultation, and on **7 April 2026 the Cabinet submitted an APPI amendment bill to the Diet** proposing new exceptions and other changes. `[verified via Baker McKenzie's 2026 client alert]` **Do not treat the current APPI as stable for 2027 planning.**
- Practical: Japan's regime is consent-heavy for third-party provision but comparatively permissive for **internal security use of pseudonymised data**, which is exactly this design.

### 6.5 Türkiye (KVKK)

- Law No. 6698; **Art 6** treats biometric data as sensitive and requires explicit consent as a rule.
- **KVKK published a Cookie Practices Guide (Çerez Uygulamaları Hakkında Rehber)**, and the Board's decision of **10.03.2022 (No. 2022/229)** addressed cookie use and the transfer of information obtained via cookies. `[verified via KVKK's own PDF URL and secondary sources describing the 2022/229 decision.]`
- **Art 9** cross-border transfer: 2024 reforms introduced new transfer mechanisms (adequacy decision, appropriate safeguards, standard contract, binding corporate rules). Practical consequence: if you store fingerprints on a non-Turkish server, KVKK transfer rules apply to Turkish users.
- `[I did not read the KVKK guide in full; treat the Turkish specifics as orientation and verify the Art 9 mechanics before launch if you have Turkish users.]`

### 6.6 Saudi Arabia and UAE

- **Saudi PDPL** — Royal Decree No. M/19 (9/2/1443H), **amended by Royal Decree No. M/148** (5/9/1444H), with **SDAIA Executive Regulations** (`sdaia.gov.sa/en/SDAIA/about/Documents/ExecutiveRegulations.pdf` — verified URL). Enforcement became active after the grace period ended (2024), with compliance deadlines extending into 2026. Secondary sources describe consent-based processing, explicit consent for sensitive data, **72-hour breach notification**, and data-localisation/transfer rules. `[The Executive Regulations PDF URL is verified; the operational specifics are from secondary sources and are marked unverified.]`
- **UAE PDPL** — Federal Decree-Law No. 45 of 2021, in force **2 January 2022**; biometric data is treated as sensitive and generally requires consent or a statutory exception; large-scale sensitive processing points toward a DPO; cross-border transfers require an adequate destination or contractual safeguards. `[secondary sources; the implementing-regulations status is the thing to verify.]`
- **Free zones have their own regimes** (DIFC DP Law 2020, ADGM). If you host in a free zone, that law may apply instead of the federal PDPL.
- Practical for a Quran app: Saudi/UAE users are likely to be a large share of your audience, and **keeping fingerprint storage in-region or in the EU with a documented transfer basis** is the low-risk option.

### 6.7 Religious/legal considerations for a Quran service — the highest-severity item

`[This is my analysis, and I flag it as the most likely thing to be missed. Get it reviewed.]`

- **GDPR Art 9(1)** prohibits processing of "personal data revealing **religious or philosophical beliefs**" unless an Art 9(2) condition applies. The CJEU reads "revealing" broadly: **C-184/20, OT v Vyriausybės** (1 Aug 2022) — indirect revelation suffices; **C-394/23, Mousse** (9 Jan 2025) — even a customer title (M./Mme) reveals sex, so treating it as non-sensitive was wrong. `[cited from knowledge of the rulings; not fetched this session.]`
- **The fact of using a Quran app is data from which religious belief can be inferred.** Therefore:
  - the _security_ processing (device key, ban record) is arguably not processing _for the purpose of_ revealing belief — but the dataset as a whole (this device → this account → this app) is belief-revealing data.
  - the safest posture is: **do not create any dataset that connects a device identifier to religious-content behaviour.** The device key must never be joined to reading history, bookmarks, favourite translations, or search terms.
  - if you ever build a "recitation streak" or "reading progress" feature, keep it in a separate store that the security system cannot query.
- **Possible Art 9(2) conditions** if the processing is genuinely Art 9 processing: **Art 9(2)(d)** — "processing relates to personal data which are **manifestly made public by the data subject**" (weak; depends on the user publicly self-identifying, cf. C-446/21 Schrems); **Art 9(2)(g)** — substantial public interest, which requires a Member State or Union law basis (you will not have one as a small app). `[analysis: neither is comfortable. The right answer is to avoid the Art 9 trigger by design, not to find a condition.]`
- **Where you have a real statutory hook, use it:** **LGPD Art 11(II)(g)** for Brazilian users (fraud prevention + security in identification/authentication), and the **UK PECR Sch A1 ¶4(2)(b)–(e)** for UK users. For EU users, the ePrivacy strictly-necessary analysis carries the load, and the Art 9 analysis must be documented as "we do not process belief data for any purpose; the security identifier is not linked to content or inferred belief".
- **Also flag in the privacy policy** (in plain words) that the app serves religious content and that you do not infer or use religious belief for any purpose. That sentence is cheap and it is the kind of thing a DPA reads first.
- **One more practical point:** a non-commercial religious service has an unusually strong "reasonable expectations" argument for security processing (users expect a service that is attacked by scrapers and harassers to defend itself) and an unusually weak one for anything else. Use that asymmetry deliberately.

### 6.8 Minors — the extra duties that actually arise

If minors can use the app, the following attach, in rough order of severity:

1. **India — hard prohibition on monitoring children.** DPDP s.9(3): no tracking or behavioural monitoring of children, no targeted advertising to children. Under-18 = child. Verifiable parental consent (Rule 10) is required before processing a child's data (in force 13 May 2027). **There is no security exemption in the Fourth Schedule.** → Strongest argument for an **age-gated account system** (no accounts for under-18s, or a fully separate no-tracking mode).
2. **Brazil — parental consent for children, with "all reasonable efforts" to verify.** LGPD Art 14. Note the terminology: Art 14's caput covers _crianças e adolescentes_ (children and adolescents) but the parental-consent paragraph refers to _crianças_; Brazilian commentary is split on whether the consent duty reaches 18. `[unverified; verify before relying.]`
3. **US — COPPA, under 13.** The FTC's amended COPPA Rule was finalised **16 January 2025**, effective **60 days after Federal Register publication** (~~**23 June 2025**), with one year from publication to reach full compliance on amendments without earlier dates (~~**23 April 2026**). What changed, from the FTC's own release:
   - **separate verifiable parental consent** to disclose a child's PI to third parties for targeted advertising;
   - **retention only "as long as reasonably necessary to fulfill a specific purpose"** — the rule "explicitly states that operators cannot retain the information indefinitely";
   - the definition of personal information was **expanded to include biometric identifiers and government-issued identifiers**;
   - Safe Harbor transparency obligations (public membership lists, more reporting).
     → Consequence: if under-13s can use the app, the device key is a persistent identifier tied to a child, and your retention schedule must be defensible under the "reasonably necessary" standard. `[verified URL: the FTC press release I fetched.]`
4. **EU/UK — Art 8 GDPR consent for information society services.** Default age **16**, but Member States may lower it to **13** (Art 8(1)). Check your lead Member State. The EDPB LI guidelines have a dedicated chapter on children's data and note that Art 6(1)(f) "in particular where the data subject is a child" is expressly weakened by the text of Art 6(1)(f) itself.
5. **UK — AADC + DUAA s.81** ("children's higher protection matters" in data protection by design). Plus the Online Safety Act's age-assurance duties for certain services. `[I did not verify the current OSA age-assurance commencement status.]`
6. **Practical recommendation** `[analysis]`: **do not fingerprint logged-out visitors at all**; require an account for anything that stores a device key; and either (a) gate accounts at 16+ with an honest declaration (which is weak), or (b) build the app so that the security fingerprint is not applied to accounts flagged as minors, or (c) avoid accounts entirely for minors and keep the reader fully anonymous. Option (c) is the only one that removes the Indian s.9(3) problem rather than documenting around it.

---

## 7. AI / automated decision-making: explainability, human review, appeal

### 7.1 GDPR Art 22 + WP251

Covered in §2.2. The operative safeguards if Art 22 applies: human intervention, express your point of view, contest the decision. Plus Art 13(2)(f)/14(2)(g)/15(1)(h) information duties about the logic, significance and consequences of automated decision-making, which the Uber case was partly about.

**The Uber decision is the reference point.** `[verified: CNIL + EDPB]` It fined €824,990,000 for Art 22 + Art 13 breaches where temporary deactivations (fraud suspicion) and temporary/permanent deactivations (low ratings) were made with **"l'absence totale d'intervention humaine dans le processus de décision"**, causing loss of income. Under appeal.

**Design implication** `[analysis]`: build the ban pipeline so that it structurally cannot be fully automatic:

- Fingerprint/abuse signals → **risk score + reason codes** → **queue**.
- **Automated action allowed:** rate-limiting, CAPTCHA/ challenge, temporary session invalidation, blocking the abusive _action_. These are proportionate, reversible, and not "decisions about the person".
- **Automated action not allowed:** permanent account termination, permanent device ban, or anything that removes access to the reader for good.
- **Human review required** before any permanent ban, with: the reason codes shown to the reviewer, the raw signals available, a written rationale recorded, and a timestamped reviewer identity.
- **Appeal channel** that reaches a human, with a stated response time. Publish it in the privacy policy.
- **Log everything** about the decision (Art 5(2) accountability): who, when, what signals, what reason code, what was decided, what the appeal outcome was. This doubles as your DSAR answer.

### 7.2 DSA — does it apply, and which articles bite?

`[verified from the article texts.]`

- **Scope:** Regulation (EU) 2022/2065 applies to intermediary services offered to recipients **in the Union** (Art 2(1)); it applies to a non-EU provider if the service is offered in the EU.
- **Is a Quran reader a "hosting service"?** Only if it stores information provided by recipients at their request. A read-only reader with accounts is **not** a hosting service for third-party content → **Art 17 does not apply**. If you add comments, notes visible to others, or user-submitted content, you become a hosting service and **Art 17 applies**.
- **Art 17 (Statement of reasons)** — applies to providers of **hosting services**, for restrictions imposed on the ground that the information is illegal or incompatible with your T&Cs, covering: restricting visibility of content; suspension/termination of monetary payments; **suspension or termination of the service in whole or in part**; **suspension or termination of the recipient's account**. It applies "where the relevant electronic contact details are known", "at the latest from the date that the restriction is imposed, **regardless of why or how it was imposed**", and must contain: what the decision was and its scope/duration; **the facts and circumstances relied on**; "where applicable, information on the use made of **automated means** in taking the decision"; the contractual ground relied on and why; and "clear and user-friendly information on the possibilities for redress".
- **Art 20 (Internal complaint-handling)** — providers of **online platforms** must give recipients access to a free, electronic internal complaint-handling system for at least **six months** after the decision, covering decisions to suspend or terminate the service or the account. Art 20(4) requires reversal "without undue delay" where the complaint contains sufficient grounds. **Art 20(6): "Providers of online platforms shall ensure that the decisions ... are taken under the supervision of appropriately qualified staff, and not solely on the basis of automated means."** ← This is the DSA's answer to automatic bans.
- **Art 21** — out-of-court dispute settlement for the same decisions.
- **Art 19 (Exclusion for micro and small enterprises)** — verbatim: "This Section, with the exception of Article 24(3) thereof, shall not apply to providers of online platforms that qualify as **micro or small enterprises** as defined in Recommendation 2003/361/EC." Section 3 contains Arts 20–28. → **A micro/small platform is exempt from Art 20 (internal complaint-handling) and Art 21 (out-of-court dispute settlement).** But **Art 17 sits in Section 2 and is not exempted by Art 19.** `[analysis: so a micro hosting service still owes statements of reasons, but not a complaint-handling system. And a read-only app owes neither.]`
- **Practical:** if the app never hosts third-party content, DSA exposure is nil. If it adds comments later, add Art 17 statements of reasons at that moment. Keep the "is this a hosting service?" question in the design doc as a trigger for re-assessment.

### 7.3 EU AI Act — is a fingerprinting ban system high-risk, or "remote biometric identification"?

`[verified from the consolidated/amended article texts on artificialintelligenceact.eu. Note: this mirror reflects the AI Act as amended by the Digital Omnibus; I read the amended text, not EUR-Lex.]`

- **Annex III (high-risk list) has no "content moderation" category.** The list is: 1. Biometrics; 2. Critical infrastructure; 3. Education and vocational training; 4. Employment, workers' management and access to self-employment; 5. Access to and enjoyment of essential private and public services (creditworthiness — **"with the exception of AI systems used for the purpose of detecting financial fraud"**; life/health insurance pricing; emergency call classification); 6. Law enforcement; 7. Migration, asylum, border control; 8. Administration of justice and democratic processes. **There is no general content-moderation or anti-abuse entry.**
- **Annex III point 1 (Biometrics):**
  - (a) **remote biometric identification** systems — defined in Art 3(35) as "the automated recognition of **physical, physiological, behavioural, or psychological human features** for the purpose of establishing the identity of a natural person by comparing biometric data ... to biometric data of individuals stored in a database". Note the carve-out: this "shall not include AI systems intended to be used for **biometric verification** ... the sole purpose of which is to confirm that a specific natural person is the person he or she claims to be."
  - (b) biometric categorisation by sensitive/protected attributes;
  - (c) emotion recognition.
- **Is a device/browser fingerprint "biometric data"?** Art 3(34) (and GDPR Art 4(14)): "personal data resulting from specific technical processing relating to the physical, physiological or **behavioural** characteristics of a natural person". A browser fingerprint is derived from _device and browser configuration_, not from a person's characteristics. The standard reading is that it is **not** biometric data and therefore not RBI. `[analysis. The counter-argument exists — some behavioural-biometric techniques (keystroke dynamics, gait) do qualify, and the EDPB/EDPS have pushed for broad "behavioural biometrics" coverage. If you ever add keystroke-timing or mouse-dynamics analysis, you cross into a genuinely contested area. Do not.]`
- **Therefore:** a fingerprint-driven abuse/ban system is **not prohibited (Art 5) and not high-risk (Art 6/Annex III)** on the current classification. `[analysis]`
- **Timing — this is important and has moved.** Art 6 as amended states: "**Comes into force 2 December 2027 (high-risk under Annex III) / 2 August 2028 (high-risk under Annex I)**, according to Article 113(c)." So Annex III high-risk obligations were **pushed back**. Also new in the amended text: Art 6(1a)–(1c) (narrowing "safety component"), **Art 4a** (processing of special categories of personal data for bias detection and correction), Art 60a, and Arts 75a–75d. The Commission's **Digital Omnibus** package is what did this; the EDPB/EDPS gave a joint opinion on it (Joint Opinion 2/2026, 11 Feb 2026).
- **Scope exemptions that may help a small open-source project** (Art 2, verified verbatim):
  - **Art 2(12):** "This Regulation does not apply to AI systems released under **free and open-source licences**, unless they are placed on the market or put into service as **high-risk** AI systems or as an AI system that falls under **Article 5 or 50**."
  - **Art 2(10):** "This Regulation does not apply to obligations of deployers who are natural persons using AI systems in the course of a **purely personal non-professional activity**."
  - **Art 2(7):** the Regulation "shall not affect Regulation (EU) 2016/679 ... or Directive 2002/58/EC" — i.e. the AI Act does not displace GDPR/ePrivacy.
- **Practical conclusion** `[analysis]`: the AI Act is very unlikely to create duties for this system. **Do not market or describe it as "biometric identification" or "remote biometric identification"** — that framing is both legally risky and factually wrong. Describe it as device-level abuse detection. If you open-source the anti-abuse component, Art 2(12) is an additional shield.
- **Also worth noting:** the AI Act's amended Art 2(7) plus the new Art 4a show the legislator explicitly carving out space for GDPR special-category processing for bias detection — a sign the Act is not aimed at this kind of system.

---

## 8. Practical compliance design patterns used by security vendors

`[All vendor claims below are from their own published docs and are cited as such — they are vendor self-descriptions, not regulator findings.]`

### 8.1 Purpose limitation expressed in code, not prose

- **Fingerprint.com** documents the trigger-on-demand pattern: "Instead of identifying visitors automatically on page load, you can choose to **explicitly trigger** the Fingerprint identification request only in specific circumstances. For example, only after the visitor grants consent by clicking 'Accept' on a cookie consent banner." `[docs.fingerprint.com/docs/privacy-and-compliance]` → The architectural lesson: **make the fingerprint call an explicit, named function that can only be invoked from the two or three code paths that need it**, and put the purpose in the function name (`identifyDeviceForAbuseCheck()`). This is what makes the ICO's "sole purpose" requirement auditable.
- **Separate the identifier from the purpose.** Fingerprint.com notes its visitor ID "is **scoped to each Fingerprint workspace**, not the internet at large" — two customers identifying the same device get different IDs. That is a purpose-limitation guarantee implemented in the key derivation. `[same source]`
- **Cloudflare Turnstile** describes its own data handling as: "Turnstile performs client-side security challenges on behalf of the website operator to distinguish human visitors from automated traffic. To do so, **Turnstile processes only the data strictly necessary to provide this security function.** Turnstile does not access, store, or transmit user communications, form entries, or other page inputs." And it publishes a **separate privacy addendum** for the product: "For detailed information on Turnstile's data privacy practices, refer to the Turnstile Privacy Addendum." `[developers.cloudflare.com/turnstile/ + cloudflare.com/turnstile-privacy-policy/]` → **Pattern: a separate, product-specific privacy addendum** is the cleanest way to tell users (and a DPA) that the security component has a different, narrower purpose than the rest of the service. Copy this: a short "Device security & abuse prevention" addendum linked from the privacy policy.
- **Cloudflare's challenge architecture** also illustrates the "do the check at the edge, before application data" idea that CNIL's LINC described: TCP/TLS fingerprints can be computed by a CDN/WAF "before the first exchange of application data," while HTTP/browser-level analysis "must therefore be performed on the server hosting the website." `[linc.cnil.fr/en/fingerprinting]`
- **Stripe Radar** shows how a vendor productises exactly this: it documents "Customer account evaluations: Detect **multi-accounting** and account sharing abuse at signup and login", "Account farming prevention: Detect coordinated account farming", and "Bot abuse prevention: Evaluate whether a payment was likely made by a bot." `[docs.stripe.com/radar]` → Note that these are named, enumerated, purpose-scoped features. Enumerate your own purposes the same way.
- **LexisNexis ThreatMetrix / Sift / Arkose / hCaptcha** all run the same shape of service (device intelligence + global reputation). `[I did not fetch their trust centres in this session — if you cite a vendor pattern for the design doc, fetch the specific page.]`

### 8.2 Pseudonymisation + peppered HMAC — and the ICO's exact words

**ICO pseudonymisation guidance** (`ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/data-sharing/anonymisation/pseudonymisation/` — verified URL) is the citable authority:

- The ICO's glossary defines **Pepper**: "a secret value added to the input, like a password, during the hashing process. A pepper is stored **separately, often in a different medium**, unlike a 'salt', which is stored with the hashed passwords in a database." `[verified via ICO glossary search result]`
- The ICO treats as **weak/insufficient** pseudonymisation: "hashing algorithms that do not use additional data to generate a pseudonym (eg a **salt, pepper** or encryption key); or outdated hashing algorithms (eg **MD5 and SHA-1**)." `[verified via search snippet of the ICO pseudonymisation page]`

**Concrete design** `[analysis]`:

- `device_key = HMAC-SHA-256(pepper, canonical_fingerprint_string)`.
- **Pepper lives in a KMS/secret manager, never in the database, never in the same backup.** Rotate it; rotation re-derives keys and can be used as a deliberate "forget everything" event.
- **Use a version prefix** (`v1:` / `v2:`) so rotation is unambiguous and old keys can be purged.
- **Salt ≠ pepper.** A per-record salt prevents rainbow tables but does not prevent cross-record linkage if the DB leaks. The pepper is what makes the store non-reversible without the KMS.
- **Never store the raw fingerprint string** — not in logs, not in error traces, not in the abuse-review UI. Store the key and the _reason codes_, not the attribute values.
- **This is pseudonymisation, not anonymisation.** Under the EDPB's pseudonymisation guidelines (Guidelines 01/2025, consultation Jan–Mar 2025; final version now published) and the ICO's guidance, a peppered HMAC is still personal data because you (the controller) hold the means of re-identification for a live session. `[verified that Guidelines 01/2025 exists and has a final version; I did not read it.]` So do not claim "anonymised".

### 8.3 Field-level encryption of identifiers

- Encrypt the device key at rest with a separate KMS key from the rest of the database (envelope encryption), so a dump of the main database does not yield usable device keys.
- Keep the **decryption permission** limited to the abuse-review service identity, not to the general application role.
- `[Cite as patterns: OWASP Cryptographic Storage Cheat Sheet (key management, rotation), OWASP ASVS v5.0 (data-protection chapter), NIST SP 800-57 Part 1 (key management), ISO/IEC 27001:2022 Annex A 8.24 (use of cryptography) and A.8.10 (information deletion). I did not fetch these in this session — they are stable documents; verify section numbers before quoting.]`

### 8.4 Data minimisation at collection time

- Compute the fingerprint **server-side from request headers** where possible (User-Agent, Accept-Language, TLS/JA4 signature via your proxy) instead of running client-side JS probes. CNIL's LINC notes the transport-layer approach requires **no client-side execution** and can be done at the edge. This removes the entire class of "intrusive client-side collection" concerns (fonts, WebGL, plugins) that the LINC article describes as "more intrusive collection of client information [that] makes it possible to identify the user in an almost unique way."
- If you do need client-side signals, collect a **fixed, documented, small set** and hash them; do not collect canvas/WebGL/audio fingerprints (high entropy, high creep factor, and the thing journalists write about).
- **Never collect precise geolocation.** Coarse (country) only, and only if needed for triage.
- Apply Art 25(2)/CPPA §7002 logic: the default configuration must be the minimised one, and there must be no toggle that silently widens it.

### 8.5 Retention timers baked into the schema

- Every table holding a device key gets `created_at` **and** `expires_at` **NOT NULL**, with a DB-level check constraint (`expires_at > created_at`).
- A scheduled job deletes `WHERE expires_at < now()`. Log the deletion count (this is your Art 5(2) evidence).
- `[Patterns to cite: PostgreSQL row-level TTL patterns / `pg_cron`+ partitioned tables with`DROP PARTITION` for cheap bulk deletion; cloud-native TTL features (DynamoDB TTL, Cloudflare D1/KV expiry, BigQuery partition expiration) which enforce expiry without application code. NIST SP 800-88 Rev. 1 for media sanitisation if you need a deletion standard.]`
- **Deletion must reach backups.** Document the backup rotation window and state that expired keys are not restored into the active store (this is the same problem the DPDP Rules commentary flags for Indian data fiduciaries: "erased data should not return to active processing through restoration").

### 8.6 DSAR tooling for device records

- You need to be able to answer, for one data subject: "what device keys do you hold that are linked to me, why, since when, and what was decided about them." So the schema must support `user_id ↔ device_key` lookup **and** orphan device keys (bans with no account).
- Two query paths: by account (easy) and by device (hard, and honestly you cannot verify a DSAR request by device). Document that a device-only record is not searchable by a natural person and explain how you handle it.
- Cite the ICO right-of-access guidance and, for tooling patterns, vendor DSAR features (Transcend, OneTrust DSAR, MineOS) — `[not fetched in this session]`.
- **Design consequence:** if you cannot service a DSAR for a device record, that is a strong argument for short retention and for keeping device records unlinked from accounts wherever possible.

### 8.7 Audit trails for admin lookups

- Every read of a device key by a human (abuse reviewer, admin, support) writes an **append-only audit row**: who, when, which key, what they looked at, what they did, ticket/reason reference.
- Make the audit log write-through and tamper-evident (append-only table, no UPDATE/DELETE grants for the app role; ideally hash-chained).
- Review access periodically (who has the decryption permission, and did they use it).
- `[Cite: ISO/IEC 27001:2022 A.8.15 (logging) and A.8.16 (monitoring activities); NIST SP 800-92 (log management). Verify clause numbers before quoting.]`

### 8.8 The vendor-side legal hygiene that most small projects miss

If you use a third-party fingerprint/CDN vendor at all:

- Get the **DPA** signed, and get the vendor's role stated as **processor/service provider** (not joint controller).
- Contractually forbid: cross-customer data pooling; vendor-side model training on your data; any vendor use for advertising; retention beyond your stated period.
- Check the vendor's **sub-processor list** and **transfer mechanism** (SCCs + transfer impact assessment) if data leaves the EU/UK.
- Verify the vendor honours **deletion requests** ("Fingerprint must delete personal information it receives from a data controller when requested" — `docs.fingerprint.com/docs/privacy-and-compliance`).
- **The safest option for this project is no third-party fingerprint vendor at all.** A server-side TLS/HTTP header fingerprint + a peppered HMAC in your own database avoids the whole "is this a sale/share?" question under CCPA, avoids SCCs, avoids the sub-processor chain, and avoids a vendor's breach becoming your breach. `[analysis — and it is also cheaper.]`

---

## 9. Concrete recommendation for this app

### 9.1 Design in one paragraph

Collect a **small, fixed set of request-level signals server-side** (User-Agent, Accept-Language, Accept-Encoding, TLS/JA4 signature, HTTP protocol version, plus a session cookie you control). Derive `device_key = HMAC-SHA-256(pepper_vN, canonicalised_signals)`. Store **only the key**, never the raw signals. Use it for exactly three purposes: (1) **session/account abuse detection** (multi-account, ban evasion, scraping), (2) **optional trusted-device login** if the user explicitly asks for it, (3) **aggregate rate-limiting**. Never join the key to reading history, bookmarks, translations, or search terms. Never use it for analytics or advertising. Never sell, share, or pool it. Expire it. Give every permanent ban a human review and an appeal. Say all of this in a two-page device-security addendum.

### 9.2 What to collect

| Collect                                                  | Why                                           | Notes                                               |
| -------------------------------------------------------- | --------------------------------------------- | --------------------------------------------------- |
| `User-Agent` (full string, hashed into the key)          | coarse device/browser class; bot detection    | hash it; never display raw UA in the abuse UI       |
| `Accept-Language`, `Accept-Encoding`, `Connection`       | consistency checks vs. UA                     | CNIL/LINC names these explicitly                    |
| TLS fingerprint (JA3/JA4-style) via your reverse proxy   | catches scripted clients, VPN/proxy patterns  | computed at the edge, before app data; no client JS |
| HTTP protocol version + header order                     | low-entropy, useful signal                    | —                                                   |
| Your own first-party session cookie (httpOnly, SameSite) | stable, consent-free session continuity       | random, not derived from the fingerprint            |
| Account ID (only when logged in)                         | links the key to a user for DSAR/ban purposes | stored in a separate column, access-controlled      |
| Ban/abuse reason codes + timestamps + reviewer ID        | Art 22 / DSA Art 17 / LGPD Art 20 compliance  | —                                                   |

### 9.3 What NOT to collect

- Canvas, WebGL, AudioContext, font-list, plugin, screen-size-and-DPI, timezone, battery, media-device enumeration — the "high-entropy browser fingerprint" set. Not needed for abuse triage and it is the set that makes headlines.
- Precise geolocation. Ever.
- Raw IP addresses persisted with the device key. If you need IP for rate limiting, keep it in a **short-lived, separate** store (hours) and never join it to the device key. `[analysis: the EDPB says IP-only tracking can trigger Art 5(3); keeping IP out of the long-lived store avoids that entirely.]`
- Any content-derived data (surah/ayah IDs, search terms, bookmarks, recitation data) in the same store as the device key. This is the §6.7 rule and it is non-negotiable.
- Any behavioural biometric (keystroke timing, mouse dynamics). Contested classification; no benefit here.
- Cross-site or cross-device signals. PECR Sch A1 + ICO: consent required, and it breaks the security exception.

### 9.4 What to hash and how

- `device_key = "v1:" + base64(HMAC-SHA-256(pepper, canonical(signals)))`, truncated to 128 bits (plenty; shorter is worse).
- Pepper in KMS/secret manager; separate from the DB; rotatable; `vN` prefix so rotation is clean.
- If you want per-record unlinkability in a leak, add a per-deployment salt **in addition** to the pepper (salt prevents cross-deployment correlation; pepper prevents reversal).
- Do not use a plain hash. ICO: hashing without a salt/pepper/key is weak pseudonymisation. Do not use MD5/SHA-1.
- Document in the privacy policy that it is **pseudonymised, not anonymous**.

### 9.5 How long to keep it

| Record                                               | Retention                                                         | Enforced by                                        |
| ---------------------------------------------------- | ----------------------------------------------------------------- | -------------------------------------------------- |
| Raw signals (UA, headers, TLS sig)                   | **not persisted** — compared in memory, discarded                 | architecture                                       |
| `device_key` for session integrity                   | **30 days**                                                       | `expires_at` column + scheduled job                |
| `device_key` for trusted-device login                | session + **30 days**, deleted on withdrawal                      | `expires_at` + account settings action             |
| Ban record (`device_key` + reason + date + reviewer) | **12 months**, then reduce to a **count-only** aggregate (no key) | scheduled job that replaces the row with a counter |
| Audit log of admin lookups                           | **12 months**                                                     | `expires_at`                                       |
| Aggregate counters (no identifier)                   | indefinite — not personal data if genuinely non-linkable          | —                                                  |

If you later observe that repeat abuse genuinely returns after 12 months, extend to 24–36 months **and document the observation**. Do not start at 3 years.

### 9.6 What to tell users (privacy policy + a separate addendum)

Two documents, not one:

**A. "Device security and abuse prevention" addendum** (linked from the privacy policy, ~1 page, plain language):

- What we compute: a small set of request signals, hashed with a secret into a device key.
- Why: to stop abuse, multi-accounting, ban evasion and scraping.
- What we do **not** do with it: no advertising, no analytics, no profiling of what you read, no selling or sharing, no cross-site tracking.
- How long: 30 days for the key; 12 months for a ban record; then aggregate only.
- Legal basis: legitimate interests (Art 6(1)(f)) / security (PECR Sch A1 ¶4(2)) / fraud and authentication security (LGPD Art 11(II)(g)); and consent where required (India, and anywhere we cannot rely on the security exception).
- Your rights: object, erasure, and how to reach us.
- Human review and appeal for any permanent ban.
- Explicit statement: **"We do not infer, record, or use your religious beliefs for any purpose. Our security records are not linked to what you read or listen to."** (§6.7)

**B. Privacy policy** — the normal Art 13/14 content, with a table of purposes/legal bases/retention, plus a **retention column** for every category. CPPA §7012 requires retention disclosure at collection for California; the DPDP Rules Rule 3 require an itemised notice for India. Write one table that satisfies all of them.

### 9.7 What UI to add

1. **No banner on the reader.** No fingerprint on page load. Nothing fires until the user does something that needs it.
2. **Account settings → "Security & devices"** page: shows the trusted devices, a "Forget this device" button that deletes the key immediately, and a one-line explanation.
3. **Just-in-time panel** only on the two paths that need it: signup/login (if you offer trusted-device login) and "report abuse". Purpose-specific, Accept/Decline of equal weight, Decline costs you nothing but the convenience.
4. **Ban notice** with: what happened, why (reason code in plain language), what signals were used, that a human reviewed it, how to appeal, and a link to the addendum.
5. **Appeal form** that reaches a human, with a stated response time.
6. **A short "how we stop abuse" page** (public) — this is the artefact that answers the "sole purpose" question for a DPA in one read.
7. **Consent-gated mode for regions/users where consent is the required basis** (India, and children under the applicable age). If consent is declined: no fingerprint, rate-limiting only, reader fully usable.

### 9.8 Document these five artefacts (this is what makes it defensible)

1. A **legitimate-interests assessment (LIA)** — three-step EDPB structure (interest → necessity → balancing), including the "less intrusive alternative" analysis (why rate-limiting + session tokens alone are insufficient).
2. A **short DPIA** — even if arguable, it is cheap and it is the best evidence of the balancing test. Use the EDPB's 2026 DPIA template.
3. A **retention schedule** with the justification for each period.
4. A **purpose-limitation statement in code**: a comment at the top of the fingerprint module, plus a lint/test that fails if the module is imported from anywhere other than the two allowed call sites. `[analysis — this is the single most persuasive technical artefact, because it makes the ICO's "sole purpose" requirement machine-verified.]`
5. A **decision log entry** for the ban system stating that permanent bans require human review, with the appeal path.

---

## 10. Do this / Don't do this checklist

### Do this

- [ ] **Treat fingerprinting as in scope of Art 5(3)/PECR** and write the analysis down. Cite EDPB Guidelines 2/2023 ¶2 and Planet49 ¶70.
- [ ] **Rely on the security exception only if the purpose is genuinely singular.** One identifier, one purpose. Document it. (ICO: use it for another purpose and the exception is gone.)
- [ ] **Do the fingerprint computation server-side** from request headers/TLS. Avoid high-entropy client-side probes.
- [ ] **Hash with a peppered HMAC-SHA-256**, pepper in KMS, versioned keys, rotatable.
- [ ] **Never persist raw fingerprint signals.** Compare and discard.
- [ ] **Never join the device key to reading/recitation/search data.** (§6.7)
- [ ] **Never collect precise geolocation.** Coarse country at most, not persisted with the key.
- [ ] **Put `expires_at NOT NULL` on every table holding a device key** and enforce with a scheduled job.
- [ ] **Use 30 days / 12 months / aggregates** as the retention ladder; extend only with documented evidence.
- [ ] **Require human review before any permanent ban.** Reason codes + written rationale + reviewer identity + appeal channel.
- [ ] **Make automated enforcement reversible**: rate limit, challenge, session invalidate. Not terminate.
- [ ] **Publish a separate device-security addendum** and a plain-language "how we stop abuse" page.
- [ ] **State explicitly that you do not infer or use religious belief.** (§6.7)
- [ ] **Write a legitimate-interests assessment and a short DPIA.**
- [ ] **Enforce purpose limitation in code** — the fingerprint module callable only from the allowed call sites.
- [ ] **Audit-log every human read of a device key**, append-only.
- [ ] **If you add "remember this device" login, prefer a random httpOnly token over the fingerprint.** Use the fingerprint for abuse signals only.
- [ ] **If you use a vendor, make it a processor/service provider**, forbid pooling and model training, sign the DPA, check transfers.
- [ ] **Re-check the DSA hosting-service question** the moment you add user-generated content.
- [ ] **Re-check the CCPA "business" thresholds** the moment you monetise or share anything.
- [ ] **Document the India position explicitly**, because DPDP has no security basis and no ePrivacy carve-out.
- [ ] **Decide the minors question explicitly** — no accounts for under-18s is the only clean answer for India's s.9(3).

### Don't do this

- [ ] **Don't use Art 6(1)(d) vital interests** for routine abuse prevention.
- [ ] **Don't fingerprint logged-out visitors.**
- [ ] **Don't reuse the fingerprint for analytics, A/B testing, personalisation, "product improvement", or debugging.** ICO names each of these as exception-breaking.
- [ ] **Don't do cross-site or cross-device tracking.** Consent required; kills the exception.
- [ ] **Don't let a vendor pool your data across customers** — that can turn a security tool into a "share" and drag in CCPA §7025/GPC, opt-out links and §7051 contracts.
- [ ] **Don't keep IP addresses with the device key** long-term.
- [ ] **Don't claim "anonymous" or "anonymised".** Say pseudonymised.
- [ ] **Don't use MD5/SHA-1, and don't hash without a pepper.**
- [ ] **Don't make permanent bans automatic.** Uber: €824,990,000.
- [ ] **Don't describe the system as biometric identification** or as "remote biometric identification" — wrong and risky.
- [ ] **Don't add keystroke/mouse-dynamics analysis.** Behavioural biometrics is a contested classification you do not need.
- [ ] **Don't build a cookie banner for a security-only fingerprint** if the exception applies — a banner that asks for consent you don't need is worse than a clear addendum, because it implies you thought consent was required.
- [ ] **Don't put a pre-ticked box anywhere.** Planet49.
- [ ] **Don't gate the reader on consent.** Declining must never break reading.
- [ ] **Don't retain "just in case".** No expiry = the finding.
- [ ] **Don't put the pepper in the database, the repo, or the same backup.**
- [ ] **Don't forget backups** when you implement deletion.
- [ ] **Don't rely on the AI Act open-source exemption as a substitute for GDPR compliance** — Art 2(7) preserves GDPR/ePrivacy in full.
- [ ] **Don't treat DROP as applicable** (you are not a data broker) **and don't treat DROP as a substitute** for your own deletion duties.

---

## 11. Sources

### EU — legislation and CJEU

- ePrivacy Directive 2002/58/EC (consolidated, as amended by 2009/136/EC) — `https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:02002L0058-20091219` `[EUR-Lex blocked server-side fetches in this session; Art 5(3) text quoted via EDPB Guidelines 2/2023 footnote 4]`
- CJEU, _Planet49 GmbH v Bundesverband der Verbraucherzentralen_, C-673/17, 1 October 2019, ECLI:EU:C:2019:801 — para. 70 quoted via EDPB Guidelines 2/2023 ¶10. Judgment index: `https://curia.europa.eu/juris/liste.jsf?num=C-673/17`
- GDPR (EU) 2016/679 — `https://eur-lex.europa.eu/eli/reg/2016/679/oj` `[same fetch caveat]`
- EU AI Act (Regulation (EU) 2024/1689) as amended — Art 2 `https://artificialintelligenceact.eu/article/2/` · Art 6 `https://artificialintelligenceact.eu/article/6/` · Annex III `https://artificialintelligenceact.eu/annex/3/` · Art 113 `https://artificialintelligenceact.eu/article/113/` (third-party mirror reflecting the Digital Omnibus amendments)
- DSA (Regulation (EU) 2022/2065) — Art 17 `https://www.eu-digital-services-act.com/Digital_Services_Act_Article_17.html` · Art 19 `https://www.eu-digital-services-act.com/Digital_Services_Act_Article_19.html` · Art 20 `https://www.eu-digital-services-act.com/Digital_Services_Act_Article_20.html`

### EDPB

- **Guidelines 2/2023 on the technical scope of Art 5(3) ePrivacy Directive, v2.0, adopted 7 October 2024** — `https://www.edpb.europa.eu/system/files/2024-10/edpb_guidelines_202302_technical_scope_art_53_eprivacydirective_v2_en_0.pdf` (v1.0, Nov 2023: `.../2023-11/edpb_guidelines_202302_technical_scope_art_53_eprivacydirective_en.pdf`)
- **Guidelines 1/2024 on processing of personal data based on Art 6(1)(f), v1.0, adopted 8 October 2024 (draft; no final version as of Sept 2026)** — `https://www.edpb.europa.eu/system/files/2024-10/edpb_guidelines_202401_legitimateinterest_en.pdf` · consultation page `https://www.edpb.europa.eu/public-consultations/guidelines-12024-on-processing-of-personal-data-based-on-article-61f-gdpr_en`
- Guidelines 01/2025 on Pseudonymisation (consultation 17 Jan – 14 Mar 2025; final version published) — `https://www.edpb.europa.eu/public-consultations/guidelines-012025-on-pseudonymisation_en`
- **Guidelines 3/2025 on the interplay between the DSA and the GDPR** (final, 17 September 2026) — `https://www.edpb.europa.eu/documents/guideline/guidelines-32025-on-the-interplay-between-the-dsa-and-the-gdpr_en` · factsheet `https://www.edpb.europa.eu/system/files/2025-11/edpb_factsheet_gdpr_dsa_en.pdf`
- Guidelines 02/2026 on Anonymisation (open consultation 8 July – 30 Oct 2026) — `https://www.edpb.europa.eu/public-consultations/guidelines-022026-on-anonymisation_en`
- DPIA template (consultation 14 Apr – 9 Jun 2026) — `https://www.edpb.europa.eu/public-consultations/template-for-data-protection-impact-assessment_en`
- Cookie Banner Taskforce report, 18 January 2023 — `https://www.edpb.europa.eu/documents/task-force-report/report-of-the-work-undertaken-by-the-cookie-banner-taskforce_en`
- EDPB/EDPS Joint Opinion 2/2026 on the Digital Omnibus, 11 February 2026 — `https://www.edpb.europa.eu/documents/legislative-opinion/edpb-edps-joint-opinion-22026-on-the-proposal-for-a-regulation-as_en`
- EDPB topic page: Automated decision making, profiling and Online tracking — `https://www.edpb.europa.eu/topics/ai-and-technology/automated-decision-making-profiling-and-online-tracking_en`
- WP29/EDPB Guidelines WP251rev.01 on Automated individual decision-making and Profiling — `[not fetched; cite from the WP29/EDPB document index]`
- WP29/EDPB Guidelines WP248rev.01 on DPIA — `[not fetched]`

### CNIL / France

- Fingerprinting definition — `https://www.cnil.fr/fr/definition/fingerprinting`
- LINC (CNIL's innovation lab), "Fingerprinting", 2 June 2026 — `https://linc.cnil.fr/en/fingerprinting` (FR: `https://linc.cnil.fr/complement-dossier-le-fingerprinting`)
- LINC, "Prevent web scraping by blocking access to the website" — `https://linc.cnil.fr/en/article-33-prevent-web-data-extraction-blocking-access-website`
- Cookies and other tracking devices: new guidelines (19 July 2019) — `https://www.cnil.fr/en/cookies-and-other-tracking-devices-cnil-publishes-new-guidelines`
- Sheet n°15: take into account the legal basis in the technical implementation — `https://www.cnil.fr/en/sheet-ndeg15-take-account-legal-basis-technical-implementation`
- Retention durations (updated 2 April 2026) — `https://www.cnil.fr/fr/passer-laction/les-durees-de-conservation-des-donnees` · guide `https://www.cnil.fr/sites/cnil/files/atoms/files/guide_durees_de_conservation.pdf` · HR referential `https://www.cnil.fr/sites/default/files/2026-04/referentiel_durees_de_conservation_gestion_des_ressources_humaines.pdf`
- Automated decisions — Uber fine, 24 August 2026 — `https://www.cnil.fr/fr/decisions-automatisees-sanction-de-pres-de-825-millions-deuros-lencontre-duber`
- CNIL sanctions register — `https://www.cnil.fr/fr/thematique/cnil/sanctions` · `https://www.cnil.fr/fr/les-sanctions-prononcees-par-la-cnil`
- CNIL PIA tool / DPIA — `https://www.cnil.fr/en/privacy-impact-assessment-pia`

### EDPB / Dutch AP — Uber

- EDPB national news: Dutch DPA fines Uber EUR 824 990 000, 8 October 2026 — `https://www.edpb.europa.eu/news/dutch-dpa-fines-uber-eur-824-990-000-for-unlawful-automated-decision-making-and-insufficient_en`
- Dutch AP press release (linked from CNIL): `https://www.autoriteitpersoonsgegevens.nl/en/current/uber-fined-nearly-825-million-euros-for-automated-driver-blocking`

### UK

- **Data (Use and Access) Act 2025** — contents `https://www.legislation.gov.uk/ukpga/2025/18/contents` · s.112 `https://www.legislation.gov.uk/ukpga/2025/18/section/112` · Schedule 12 (inserts PECR Sch A1) `https://www.legislation.gov.uk/ukpga/2025/18/schedule/12`
- **PECR 2003, Schedule A1** (in force 5.2.2026) — `https://www.legislation.gov.uk/uksi/2003/2426/schedule/A1`
- **ICO, Guidance on the use of storage and access technologies (final, 29 April 2026)** — `https://ico.org.uk/for-organisations/direct-marketing-and-privacy-and-electronic-communications/guidance-on-the-use-of-storage-and-access-technologies/` · exceptions chapter `.../what-are-the-exceptions/` · news item `https://ico.org.uk/about-the-ico/media-centre/news-and-blogs/2026/04/final-storage-and-access-technologies-guidance-published/`
- ICO, Pseudonymisation — `https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/data-sharing/anonymisation/pseudonymisation/` · Glossary (pepper definition) `https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/data-sharing/anonymisation/glossary/`
- ICO, Profiling tools for online safety — `https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/online-safety-and-data-protection/profiling-tools-for-online-safety/`
- ICO, Age appropriate design code — `https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/childrens-information/childrens-code-guidance-and-resources/age-appropriate-design-a-code-of-practice-for-online-services/`

### United States

- Cal. Civ. Code §1798.140 (definitions; source: leginfo, Jan 1 2026) — `https://california.public.law/codes/ca_civ_code_section_1798.140` (source link to `https://leginfo.legislature.ca.gov/faces/codes_displaySection.xhtml?lawCode=CIV&sectionNum=1798.140`)
- Cal. Civ. Code §1798.145 (exemptions) — `https://california.public.law/codes/ca_civ_code_section_1798.145`
- **CPPA, CCPA Updates, Cybersecurity Audits, Risk Assessments, ADMT and Insurance Regulations** (adopted 24 July 2025; effective 1 January 2026) — `https://cppa.ca.gov/regulations/ccpa_updates.html` · approved text `https://cppa.ca.gov/regulations/pdf/ccpa_updates_cyber_risk_admt_appr_text.pdf`
- CPPA, CCPA statute & regulations effective 1 January 2026 — `https://cppa.ca.gov/regulations/` · statute `https://cppa.ca.gov/pdf/20260101_ccpa_statute.pdf` · regs `https://cppa.ca.gov/regulations/pdf/ccpa_statute_eff_20260101.pdf`
- CPPA, March 2023 CCPA regulations (incl. §7025 opt-out preference signals) — `https://cppa.ca.gov/regulations/consumer_privacy_act.html`
- **DROP (Delete Request and Opt-out Platform)** — `https://privacy.ca.gov/drop/` · DROP status `https://consumer.drop.privacy.ca.gov/dropstatus` · data broker registry `https://cppa.ca.gov/data_broker_registry/`
- FTC, **Rite Aid banned from using AI facial recognition** (Dec 2023) — `https://www.ftc.gov/news-events/news/press-releases/2023/12/rite-aid-banned-using-ai-facial-recognition-after-ftc-says-retailer-deployed-technology-without` · case `https://www.ftc.gov/legal-library/browse/cases-proceedings/2023190-rite-aid-corporation-ftc-v`
- FTC, **Everalbum, Inc.** (May 2021) — press-release URL **not verified in this session** (guessed URLs 404'd); locate via `https://www.ftc.gov/legal-library/browse/cases-proceedings`
- FTC, **COPPA Rule amendments finalised 16 January 2025** — `https://www.ftc.gov/news-events/news/press-releases/2025/01/ftc-finalizes-changes-childrens-privacy-rule-limiting-companies-ability-monetize-kids-data` · rule text `https://www.ftc.gov/legal-library/browse/federal-register-notices/16-cfr-part-312-coppa-final-rule-amendments`
- Texas Data Privacy and Security Act, Texas CUBI (Tex. Bus. & Com. Code §503.001) — `[not fetched; verify at the Texas Legislature and the Texas AG's office]`
- Oregon Consumer Privacy Act — `[not fetched; verify at the Oregon Legislature]`

### Brazil

- **LGPD, Lei 13.709/2018 (consolidated, incl. amendments to Lei 15.352/2026)** — `https://www.planalto.gov.br/ccivil_03/_ato2015-2018/2018/lei/l13709.htm` (Arts 5(II), 7(IX), 10, 11(II)(g), 11(1), 12, 13(4), 14, 15, 16, 20 read directly)

### India

- DPDP Act 2023 — `[verify at India Code / MeitY]`
- **DPDP Rules 2025, G.S.R. 846(E), notified 13 November 2025** — MeitY page `https://www.meity.gov.in/documents/act-and-policies/digital-personal-data-protection-rules-2025-gDOxUjMtQWa` (incl. Corrigendum 16.12.2025) · PIB release `https://pib.gov.in/PressReleasePage.aspx?PRID=2190655` and `https://pib.gov.in/PressReleasePage.aspx?PRID=2190014` · full text with commencement table `https://privacylawhub.com/bare-acts/dpdp-rules-2025` · Rule 8 `https://privacylawhub.com/bare-acts/dpdp-rules-2025/rule-8-time-period-for-specified-purpose-to-be-deemed-as-no-longer-being-served` · Third Schedule `https://privacylawhub.com/bare-acts/dpdp-rules-2025/schedule-iii-third-schedule-class-of-data-fiduciaries-purposes-and-time-periods` · Rule 10 `https://privacylawhub.com/bare-acts/dpdp-rules-2025/rule-10-verifiable-consent-for-processing-of-personal-data-of-a-child`

### Japan

- PPC, guidelines on anonymously processed / pseudonymised information — `https://www.ppc.go.jp/personalinfo/legal/guidelines_anonymous/` · PPC English portal `https://www.ppc.go.jp/en/`
- APPI amendment bill submitted to the Diet 7 April 2026 — `https://www.bakermckenzie.com/en/insight/publications/2026/05/japan-appi-reform-key-changes`
- PPC draft guidance on processing principles, 28 May 2025 — `https://www.dataguidance.com/news/japan-ppc-publishes-draft-guidance-how-comply`

### Türkiye

- KVKK, **Cookie Practices Guide** — `https://www.kvkk.gov.tr/SharedFolderServer/CMSFiles/fb193dbb-b159-4221-8a7b-3addc083d33f.pdf`
- KVKK guidelines index — `https://www.kvkk.gov.tr/Icerik/6618/Guidelines`
- KVKK Board decision 10.03.2022 / 2022/229 (cookies) — described at `https://www.pieravukatlik.com/yayinlarimiz/guncel-yazilarimiz/kvkk/cerez-karari/`

### Saudi Arabia / UAE

- Saudi PDPL + **SDAIA Executive Regulations** — `https://sdaia.gov.sa/en/SDAIA/about/Documents/ExecutiveRegulations.pdf` · SDAIA regulations index `https://sdaia.gov.sa/en/SDAIA/about/Pages/RegulationsAndPolicies.aspx`
- UAE PDPL (Federal Decree-Law 45/2021) — `[verify via the UAE Ministry of Justice / UAE Data Office; secondary guides: `https://www.almaazmilawyers.com/insights/pdpl-uae-data-protection-compliance-guide`]`

### Vendors / design patterns

- Cloudflare Turnstile — `https://developers.cloudflare.com/turnstile/` · Turnstile privacy addendum `https://www.cloudflare.com/turnstile-privacy-policy/`
- Fingerprint (FingerprintJS / Fingerprint Identification) — privacy & compliance `https://docs.fingerprint.com/docs/privacy-and-compliance` · regions & data retention `https://docs.fingerprint.com/docs/regions` · support article on retention `https://support.fingerprint.com/hc/en-us/articles/20792804321677-How-long-is-data-retained-by-Fingerprint` · US SPL DPA `https://docs.fingerprint.com/docs/dpa-usspl`
- Stripe Radar — `https://docs.stripe.com/radar`
- JA4+ fingerprinting library (FoxIO) — `https://github.com/FoxIO-LLC/ja4`
- FingerprintJS BotD — `https://github.com/fingerprintjs/BotD`
- `[Not fetched in this session — cite only after checking: LexisNexis ThreatMetrix, Sift, Arkose Labs, hCaptcha, Datadome, Kasada trust centres; OWASP ASVS v5.0; OWASP Cryptographic Storage Cheat Sheet; NIST SP 800-57 Pt.1; NIST SP 800-88 Rev.1; ISO/IEC 27001:2022 Annex A 8.10 / 8.15 / 8.16 / 8.24.]`

---

## 12. Uncertainty register — things I could not verify, ranked by how much they could change the design

1. **Whether fraud prevention appears in DPDP s.7 "certain legitimate uses".** If it does not (my reading), then for Indian users the security fingerprint needs consent, and the only clean escape is genuinely non-identifying data. **This is the item most likely to force a design change. Verify s.7 of the Act.**
2. **CNIL's precise 2022/2024 fingerprinting enforcement actions.** I confirmed CNIL's doctrine, its register URLs, its LINC security framing, and the 2025 sanctions themes, but I did **not** open individual fingerprint-specific decisions. Before citing "CNIL fined X for fingerprinting", check `cnil.fr/fr/themes/cnil/sanctions`. A specific CNIL fingerprinting sanction may not exist under that name.
3. **The exact wording of CNIL's Feb 2025 statement on Google's advertising fingerprinting** — the quoted sentence is from a secondary source (BFMTV). The substance is consistent with CNIL's cookie doctrine, but do not quote it as an official CNIL text without finding CNIL's own page.
4. **Planet49 paragraph text.** ¶70 is quoted verbatim through the EDPB's citation. EUR-Lex and Curia blocked server-side fetches. If the report is used externally, pull ¶¶58–70 directly from Curia.
5. **EDPB Guidelines 1/2024 is still only v1.0 (draft)** as of September 2026. Anything you build on the Art 6(1)(f) analysis should be re-checked when the final version lands, because drafts do change.
6. **The UK DUAA Schedule 4 "recognised legitimate interests" list** — I did not read it. If security/crime-prevention appears there, the UK legitimate-interest position gets materially stronger and the analysis in §1.6/§6.2 should be updated.
7. **AI Act amendments via the Digital Omnibus.** I read the amended text on a third-party mirror (`artificialintelligenceact.eu`), which shows Art 6 high-risk Annex III obligations now from **2 December 2027** and new Arts 4a/60a/75a–d. Confirm against EUR-Lex before relying on the dates.
8. **DSA micro/small-enterprise reading.** Art 19 exempts Section 3 (Arts 20–28) for micro/small online platforms. I read Art 19 and Art 17 verbatim; the conclusion that Art 17 (Section 2) is _not_ exempted is my reading of the structure, not a quoted statement.
9. **Texas TDPSA, Oregon OCPA, and all US state biometric statutes** were not fetched. The Texas/Oregon specifics in §5.6 are orientation only. Illinois BIPA in particular has a private right of action and should be checked if you ever handle real biometrics.
10. **FTC Everalbum press-release URL** — not verified. Use the FTC legal library rather than a press-release link until confirmed.
11. **Saudi and UAE operational specifics** (retention limits, transfer mechanics, DPO thresholds) come from secondary sources. The SDAIA Executive Regulations PDF URL is verified; read it directly before relying on any specific number.
12. **Japan APPI is mid-reform** (bill in the Diet from 7 April 2026). Do not treat the current regime as stable for 2027 planning.
13. **Whether LGPD Art 14's parental-consent duty reaches 18** — Brazilian commentary is split. Verify before writing an age policy.
14. **Vendor trust centres for LexisNexis/Sift/Arkose/Datadome/Kasada** were not fetched; the vendor patterns in §8 are drawn from Cloudflare, Fingerprint.com and Stripe only, all of which I did fetch.
15. **WP248 (DPIA) and WP251 (Art 22)** were not fetched in this session. Both are stable; verify the wording of the nine DPIA criteria and the "similarly significant" test before quoting.

---

_End of report._
