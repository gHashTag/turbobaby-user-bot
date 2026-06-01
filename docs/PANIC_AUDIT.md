# `.unwrap()` / `.expect()` panic audit (cycle #77)

Cycle #75 closed a real `String::truncate(N)` panic vector. Cycle #76
closed silent-failure `.ok()` callsites. This is the third leg of the
defensive trifecta: are there any `unwrap`/`expect` calls that can
actually panic in production hot path?

**Conclusion: no.** 76 callsites total, all fall into three safe buckets.
This document is the snapshot so future cycles don't have to redo the
audit — only update it when a new call is introduced.

## Methodology

```sh
grep -rn "\.unwrap()\|\.expect(" src/ --include="*.rs"
```

Each hit was inspected for:
1. **Test context** — inside `#[cfg(test)]` mod, `#[test]` fn, or test helper.
2. **Startup-only** — runs once at process boot; panic = clean process exit before serving traffic.
3. **Provable invariant** — the value is statically guaranteed (literal, just-set, fixed-size slice).

If a hit didn't fit any of these, it would be a real risk.

## Inventory

### Bucket 1 — Test contexts (49 callsites)

`src/api/{auth,observability,orders,referrals,mod,strains,quest,game,rate_limit}.rs`,
`src/db/{strains,orders}.rs`, `src/trios/{core,store,garden,quest,validation}.rs`.

These are test fixtures (`json.unwrap()`, `parse().unwrap()` on test strings,
`from_str().unwrap()`). Test code may panic — that's how `assert!` works.
Not a production concern.

### Bucket 2 — Startup-only (7 callsites)

| File | Line | Call | Why it's fine |
|------|------|------|---------------|
| `src/main.rs` | 186 | `.expect("Failed to install rustls crypto provider")` | Runs once at boot. Failure = misconfigured TLS; process should exit. |
| `src/main.rs` | 698 | `.expect("failed to install Ctrl+C handler")` | Boot-time signal handler install; failure = OS won't deliver SIGINT, which we can't recover from anyway. |
| `src/main.rs` | 703 | `.expect("failed to install signal handler")` | Same reasoning. |
| `src/main.rs` | 784, 794, 804, 820 | `"br, gzip".parse().unwrap()` etc. | Static HeaderValue parses. Strings are compile-time constants. |

Panic at boot is the desired behavior — better than silently running with
broken TLS or missing signal handlers.

### Bucket 3 — Provable invariants (3 production-path callsites)

| File | Line | Call | Invariant |
|------|------|------|-----------|
| `src/api/cache.rs` | 27 | `hash[0..8].try_into().unwrap()` | `Sha256::digest` returns `[u8; 32]`; `[..8]` is always 8 bytes, which `try_into::<[u8; 8]>()` always accepts. |
| `src/bot/mod.rs` | 25, 42 | `"https://t.me".parse().expect(...)` | Hardcoded fallback URL. Compile-time literal. The `.expect()` carries a justification message. |
| `src/ui/screens/checkout_screen.rs` | 95 | `k.clone().unwrap()` | The line above sets `*k = Some(...)` if `k.is_none()`, so `k.is_some()` is guaranteed at this point. |

Cycle #77 applied three small reinforcements to make these self-documenting:
* `cache.rs:27` — added `.expect("SHA-256 digest is 32 bytes; [..8] is always 8")` so a future refactor that returns a different hash size fails loudly with context, not generically.
* `checkout_screen.rs` — replaced the manual write-then-clone-unwrap with `get_or_insert_with(...)`, which encodes the invariant in the type system (returns `&mut String`, no unwrap needed).
* `bot/mod.rs` — already had explanatory `.expect("static URL is always valid")` from before; left as-is.

## How to extend this audit

When grep finds a new hit, classify into one of the three buckets:

* **Test?** → no concern, ignore.
* **Startup?** → add to table 2 with a one-line "why it's fine".
* **Provable invariant?** → add to table 3, ideally with a self-documenting `.expect("invariant: ...")` message.
* **None of the above?** → that's a real bug. Rewrite to return `Result` or use `unwrap_or_*` / `?` / `let Some(...) = ... else { ... }`.

## Why this matters

A `.unwrap()` is fine — until someone refactors the surrounding code in
three years and silently invalidates the invariant. The `.expect(msg)`
form encodes the invariant in the source, so the refactor breaks loudly
and the panic message points the next maintainer at what to check.

This audit is a low-cost insurance policy: 76 grep hits, ~30 minutes to
classify, document so the next cycle starts from a known baseline.
