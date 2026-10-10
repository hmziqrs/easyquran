# quran.com reference mirror

Committed mirror of quran.com's script-level assets (script JSON + fonts), used as the audit
reference for parity checks. Catalogue, licence register, scope, and production policy live in
[`docs/qurancom-mirror.md`](../../../docs/qurancom-mirror.md).

Refresh:

```bash
pnpm quran:mirror             # verify; fetch missing or size-mismatched files
pnpm quran:mirror --dry-run   # report only
pnpm quran:mirror --record    # fetch all; print sizes to rebake into the registry
```

Nothing here is served by the app; every entry is production-disabled until its licence clears
and an env-gated consumer exists.
