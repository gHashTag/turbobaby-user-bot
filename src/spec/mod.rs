//! Rules executed from `.t27` specs, not written by hand.
//!
//! Each file here is the output of `t27c gen-rust` on the spec it names and is
//! never edited. The spec's tests and invariants run on t27b
//! (`t27b test specs/turbobaby/<id>.t27`); this module is only how the host
//! calls the same functions. Regenerate after changing a spec, with t27c
//! built from gHashTag/t27 @ 3733ff1 (the pin of the `t27b-contracts` CI job;
//! `scripts/check_spec_gen.sh` fails on any byte of drift):
//!
//! ```text
//! t27c gen-rust specs/turbobaby/request_identity.t27 > src/spec/request_identity.rs
//! t27c gen-rust specs/turbobaby/pricing_honesty.t27 > src/spec/pricing_honesty.rs
//! ```
//!
//! `pricing_honesty` is also called from `src/trios/pricing.rs`, which compiles
//! for wasm32 as well (D15), where this module is not mounted; that file mounts
//! the same generated file by path on those targets, so both runtimes execute
//! one source.
//!
//! Generated code keeps the spec's own shape (explicit `return`, parentheses,
//! every constant whether the host uses it or not), so rustfmt and the lints
//! that would rewrite it are silenced for these modules alone.

#[allow(
    dead_code,
    unreachable_pub,
    unused_parens,
    clippy::all,
    clippy::needless_return
)]
#[rustfmt::skip]
pub(crate) mod request_identity;

#[allow(
    dead_code,
    unreachable_pub,
    unused_parens,
    unexpected_cfgs,
    clippy::all,
    clippy::needless_return
)]
#[rustfmt::skip]
pub(crate) mod pricing_honesty;
