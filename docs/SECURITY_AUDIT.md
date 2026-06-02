# Security audit (cycle #107)

CI runs `cargo audit` on every push and pull request. The check fails on
**any new** advisory; four advisories are explicitly ignored with
documented rationale below. Re-evaluate this file when:

- Upstream releases a fix for an ignored advisory (then drop the
  ignore and bump the dep).
- A new audit failure appears — add a real fix or add a new ignore
  here with rationale, not silently.

The audit baseline at cycle #107: 4 advisories in the lockfile, all
either in unused code paths or blocked on upstream maintainers.

## Ignored advisories

### RUSTSEC-2023-0071 — `rsa` Marvin Attack (timing sidechannel)

- **Severity**: 5.9 medium
- **Chain**: `sea-orm 1.1` → `sqlx 0.8` → `sqlx-macros` → `sqlx-mysql 0.8` → `rsa 0.9.10`
- **Why ignored**: the `rsa` crate is only used by `sqlx-mysql`. We use
  `sqlx-postgres` exclusively (see `sea-orm` features in `Cargo.toml`).
  The `rsa` code path is never invoked at runtime; it's compiled in
  only because `sqlx-macros` pulls all driver-flavoured proc-macros.
- **Fix path**: blocked on `sqlx` upstream restructuring its macro crate
  to be database-flavor-conditional, or on `rsa` shipping a constant-time
  impl. Neither has happened as of cycle #107.
- **Upstream**: <https://github.com/RustCrypto/RSA/issues/19>

### RUSTSEC-2026-0098, -0099 — `rustls-webpki` 0.101.7 name-constraint flaws

- **Severity**: medium (server-side cert chain validation)
- **Chain**: `aws-sdk-s3 1.x` → `aws-smithy-runtime` (feature `tls-rustls`)
  → `aws-smithy-http-client 1.1.12` (feature `legacy-rustls-ring`)
  → `rustls 0.21.12` → `rustls-webpki 0.101.7`
- **Why ignored**: both advisories cover edge cases in name-constraint
  enforcement during TLS cert chain validation. Triggering either
  requires connecting to a server that presents a cert chain with a
  malicious name constraint extension AND a CA that lets the constraint
  bypass slip through. Our only TLS endpoint via this chain is AWS S3,
  which uses Amazon Trust Services and Amazon-issued certs — no
  third-party CA, no name-constraint-bearing leafs.
- **Fix path**: aws-sdk-s3 has not switched its default TLS provider
  to `rustls 0.23` + `rustls-webpki 0.103`. The modern feature
  `default-https-client` would force `rustls-aws-lc` (aws-lc-rs crypto
  provider, not `ring`), creating dual crypto providers in the binary.
  Tracked separately — wait for aws-sdk-s3 to expose
  `tls-rustls-ring`-equivalent or move when dual-crypto-provider cost
  is justified.
- **Upgrade plan (cycle X)**: drop `default-features = false` +
  `features = ["rustls"]` from aws-sdk-s3 / aws-config, accept
  aws-lc-rs in addition to ring. Audit binary size + runtime cost.

### RUSTSEC-2026-0104 — `rustls-webpki` 0.101.7 reachable panic in CRL parsing

- **Severity**: medium (DoS via crafted CRL)
- **Chain**: same as -0098/-0099
- **Why ignored**: requires the application to enable certificate
  revocation list (CRL) verification in its rustls ClientConfig. The
  aws-sdk-s3 TLS path does not configure CRL — the affected code is
  never reached.
- **Fix path**: same upstream block as -0098/-0099.

## Running locally

```sh
cargo install cargo-audit  # one-time
cargo audit \
  --ignore RUSTSEC-2023-0071 \
  --ignore RUSTSEC-2026-0098 \
  --ignore RUSTSEC-2026-0099 \
  --ignore RUSTSEC-2026-0104
```

The CI step in `.github/workflows/ci.yml` uses the same flags.

## When a NEW advisory fires

1. Read the RUSTSEC URL printed by `cargo audit`.
2. Walk the dependency chain in the output.
3. Decide:
   - **Fix**: bump the offending crate (preferred). Update Cargo.toml,
     run `cargo update -p <crate>`, re-audit.
   - **Ignore**: add `--ignore RUSTSEC-XXXX-YYYY` to the CI flags +
     append a section to this doc with the same fields as above.
4. Open a PR — the CI step must pass before merge.
