# EasyQuran — Roadmap

**Monorepo + shared types**
- Workspace: `/rust`, `/flutter`, `/web` (existing)
- Rust structs → codegen → TS types + Dart types (single source of truth)
- CI: typegen on every PR

**Rust backend**
- Axum server, `/health`
- Quran content API (surahs, ayahs, translations)
- State: reading progress, bookmarks, settings (DB)

**Auth**
- Google, Apple, Facebook OAuth — no email/password
- Session-based auth (server-side sessions, no JWT)
- `users` + `sessions` tables

**Bookmarks**
- Save / list / remove bookmarks (surah:ayah)
- Last-read position (auto-resume)

**Web**
- Marketing pages — minimal copy
- `/app` — reader (open, no auth required)
- Auth only for sync (progress, bookmarks)

**Flutter**
- One codebase: mobile + desktop
- Reader + auth, reusing shared types

**Ship**
- Backend → VPS (Axum)
- Web → Cloudflare Pages / Vercel
- iOS, Play Store, macOS/Windows/Linux desktop builds

---

MVP gate = monorepo + backend + web reader live. Auth + Flutter after.
