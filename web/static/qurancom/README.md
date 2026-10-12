# quran.com reference mirror

Committed mirror of quran.com's script-level assets (script JSON + fonts), used as the audit
reference for parity checks. Catalogue, licence register, scope, and production policy live in
[`documentation/qurancom-mirror.md`](../../../documentation/qurancom-mirror.md).

Refresh:

```bash
bun run quran:mirror             # verify; fetch missing or size-mismatched files
bun run quran:mirror --dry-run   # report only
bun run quran:mirror --record    # fetch all; print sizes to rebake into the registry
```

Nothing here is served by the app; every entry is production-disabled until its licence clears
and an env-gated consumer exists.
