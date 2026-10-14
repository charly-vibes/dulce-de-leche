# Decision: secure default vs. skip-and-install

## Context

Family tools do not all publish checksums yet. The previous behavior skipped
verification when no checksum existed, so installs kept working for those tools.

## Options considered

1. **Reject unavailable verification (chosen)** — fail closed. Unverified downloads
   cannot be authenticated; installing them anyway means the checksum pass provides no
   guarantee for exactly the releases where an attacker would suppress metadata.
2. Skip-and-install with a loud warning — rejected: warnings are ignorable and, in
   JSON pipelines, invisible; the install proceeds unprotected.
3. Blocking on signature-based verification (sigstore/release signing) — deferred:
   requires coordinated changes to every family tool's release workflow. ddl-yts
   delivers the fail-closed policy now; signing can layer on top later.

## Compatibility transition

- `DDL_ALLOW_UNVERIFIED_RELEASE=1` (also `true`/`yes`) authorizes a legacy-unverified
  install for releases without usable checksum metadata. The install result is
  explicitly labeled `unverified — explicitly authorized via
  DDL_ALLOW_UNVERIFIED_RELEASE` in human output and `unverified-authorized` in JSON.
- A published checksum mismatch is NEVER bypassed by the authorization.
- Older releases that do publish checksums (ddl's own release workflow has published
  `checksums.txt` since DDL-5ph) are unaffected.
