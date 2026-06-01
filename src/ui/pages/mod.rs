// `pages/` directory historically held an early version of every screen.
// Almost all of them were superseded by `src/ui/screens/*_screen.rs` long ago
// and were never wired into `src/ui/routes.rs`. Cycle #15 hit a real 404 in
// the production app because `pages/profile.rs:50` was still building a
// stale image URL — exactly the kind of bug you get when dead code keeps
// drifting next to live code.
//
// Cycle #23 removed the 13 unused modules. Only `referrals` remains because
// `src/ui/screens/referrals_screen.rs` actually imports `Referrals` from here.
// If you find yourself adding more to this directory: don't. Add to
// `src/ui/screens/` next to the equivalent `_screen.rs` file instead.

pub mod referrals;

pub use referrals::Referrals;
