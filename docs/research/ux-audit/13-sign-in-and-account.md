# 13 · Sign in, register and account

[← Back to the index](README.md)

> **Question this page answers:** *Is signing in simple, trustworthy, and easy to back out of?*

---

### AUTH-01 · No way back from Sign in / Create account

**P1** · everyone who opens it by accident or changes their mind · `web/src/routes/(auth)/+layout.svelte`

![Sign in: no header, no back link](screenshots/auth/login.webp)

**What's wrong.** `/login` and `/register` render with no header, no logo link, no "Back to reading". On phones, the browser Back button is the only exit.

**Fix.** Add a slim header (logo → app home, "✕ Close" / "← Back to reading" on the right) and keep the reader's return URL.

---

### AUTH-02 · Sign in and Create account don't match

**P2** · `(auth)/login/+page.svelte`, `(auth)/register/+page.svelte`

![Sign in has a blue brand block; Create account doesn't](screenshots/auth/login-vs-register.webp)

**Fix.** Use one `AuthShell` for both (either both with the brand block or neither), same spacing, same title size.

---

### AUTH-03 · Social sign-in rows don't look like buttons; GitHub for this audience

**P2** · everyone

**What's wrong.** "Continue with Google / Apple / Facebook / GitHub" rows have no border or background — they read as a list of text. Four providers + email is a lot of choice; GitHub is a developer service most of the audience won't have. "OR CONTINUE WITH" is in the code font.

**Fix.** Outline pill buttons (44 px, icon left, label centred). Show Google and Apple first; move Facebook/GitHub behind "More options" (or drop GitHub). Divider text in the UI font, sentence case: "or".

---

### AUTH-04 · Small, colour-only links and missing page titles

**P2 (WCAG 1.4.1, 2.4.2)**

- "Reset it" and "Create one" are 13 px, colour-only (1.6:1 against body text), 18 px tall. Underline, raise to 15 px, give 44 px hit area.
- `/login` and `/register` have no `<title>` (axe *document-title*).
- The password field's eye icon shows "eye-off" while the password is hidden; many users read that as "hidden — tap to show" and others as "tap to hide". Add visible text "Show" / "Hide".

---

### AUTH-05 · Why sign in? The value isn't stated where it matters

**P3**

Bookmarks, Settings → Account and Sign in all say "sync across devices". For non-technical readers, lead with the benefit and the safety: "Keep your bookmarks and notes safe if you lose your phone." State clearly that reading never needs an account (the FAQ says it; the sign-in page doesn't).
