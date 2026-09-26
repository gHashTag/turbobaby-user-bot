#!/usr/bin/env python3
"""Fail the build when a .t27 contract and the Rust or SQL it constrains disagree.

43 contracts lived under specs/ when this script was written (2026-09-21; re-measured
2026-09-22: 45 .t27 files, order_presentation.t27 and person_naming.t27 having arrived
with 14b01ac). Exactly one of them was mechanically tied to the code
it describes before this script existed: scripts/verify_fleet_seed.py reads
specs/turbobaby/market_profile.t27 and refuses to pass when data/fleet_seed.json
disagrees with it. Everything else was held together by a human reading two files side
by side, and the pinned compiler cannot help -- measured 2026-09-21 with t27c at ref
40003ed1379c8a417e13e45843de35088b88f8c0, `parse --json` returns TestBlock and
InvariantBlock nodes carrying a name and NO children, so every `assert` inside a
contract is, to the whole toolchain, a comment. A contract can state 3000 while the
shipped constant says 4000 and both the compiler and the test runner stay green.

This script is scripts/verify_fleet_seed.py's pattern generalised: one declarative
table of BINDINGS, each naming a constant in a contract, a way to extract the same
fact out of a source file, and the relation that must hold between them. Adding a
binding is a few lines of table; it is never new code.

    python3 scripts/verify_t27_against_source.py            # quiet unless wrong
    python3 scripts/verify_t27_against_source.py -v         # print every binding
    python3 scripts/verify_t27_against_source.py --require-git-tracked
    python3 scripts/verify_t27_against_source.py \n        --source-override src/trios/stars_cap.rs=/tmp/planted.rs   # negative control

--source-override is local-only, is refused under CI, and is refused alongside
--require-git-tracked: the tracked check vouches for the repo-relative path in the
table, which is not the file an override makes the gate read.

Exit 0 = every binding holds. Exit 1 = at least one does not, or could not be
evaluated. There is no third outcome: a binding whose contract constant has vanished,
whose source anchor matches nothing, or whose source anchor matches more times than
declared, is RED. DECISIONS.md D16 is the rule -- a gate whose input can reach zero
must pin a floor, because "found nothing" and "found nothing wrong" look identical
from the outside. The floor on the table itself is MIN_BINDINGS.

WHAT A GREEN RUN DOES NOT PROVE. Re-measured 2026-09-25 on the integration of that day's owner
answers (items 7, 10, 11, 12 client and server, and 13): the table's 239 bindings cover 226
distinct (contract, constant) pairs out of 5698 top-level `pub const` declarations across the
45 files under specs/, touching 41 of those 45 contracts -- 4.0% of the declared constants,
and 0% of the 10246 `assert` statements. The reading of 2026-09-24 follows, as it was.
Re-measured 2026-09-24, after the owner's rental-only ruling
(nine rows over the same 41 contracts) and its review (one row: migration 088's hide) landed on
top of the spec-hygiene change: the table's 237 bindings cover 224 distinct (contract, constant)
pairs out of 5564 top-level `pub const` declarations across the 45 files under specs/, touching
41 of those 45 contracts. That is 4.0% of the declared constants, and it is 0% of the 9996
`assert` statements. (Earlier on 2026-09-24: the ruling before its review, 236 bindings, 223
pairs of 5540, 4.0%, 9958 asserts, 41 contracts; after the spec-hygiene change (the four ride-handling rows, the migration census
owner's three, the nmax-155 rate's two and the seed's line count), 227 bindings, 214 pairs
of 5484, 3.9%, 9859 asserts, 41 contracts; after the
T27 C3 deposit comparison, 217 bindings, 204 pairs of 5427, 3.8%, 9753 asserts, 41
contracts; after the Phuket delivery zones (migration 087) and their sixteen bindings, 216
bindings, 203 pairs of 5418, 3.7%, 9738 asserts, 41 contracts; the spec-hygiene change
alone on f0640f8, 208 bindings, 200 pairs of 5362, 3.7%, 9636 asserts, 41 contracts; on the
catalog-honesty tree (the CLICK 125 redirect, the customer availability line and the admin
Add answer on top of upstream main f0640f8), 200 bindings, 192 pairs of 5320, 3.6%, 9558
asserts, 41 contracts; on the tree that added the status, cancellation and money families
and the person-naming resolution of 2026-09-22/23 to the binding groups below, 198
bindings, 190 pairs of 5310, 3.6%, 9535 asserts, 41 contracts.) (2026-09-22, the binding
groups alone: 168
bindings, 164 pairs of 4997, 3.3%, 9038 asserts, 40 contracts. Earlier that day, before
a review's fixes: 162 bindings, 158 pairs of 4942, 3.2%, 8940 asserts. The fixes added
six rows, and most of the 55 constants and 98 asserts between those two readings are
dated re-measurements kept beside the readings they correct.) This script
compares DECLARED VALUES, it does not execute a
single contract predicate, and a contract whose constants all match the code can still
assert something false about them. The four files no row names as a contract are
specs/agents/turbobaby.t27 (read here only as a SOURCE), market_profile.t27 (tied by
scripts/verify_fleet_seed.py instead), person_naming.t27 and ride_runtime.t27;
order_presentation.t27 has been bound since 2026-09-22. Every binding below was measured by hand before it
was written; the rest of the corpus is unchecked and is not claimed otherwise. Growing
the table is the point; a number here that nobody re-measured is the defect this file
exists to catch. Was, re-measured 2026-09-21 11:2x after an adversarial pass: 66
bindings, 64 pairs of 4220 declarations across 43 files, 16 of 43 contracts, 1.5%,
7890 asserts. (The corpus moves: the 2026-09-21 assert figure read 7921 two hours
earlier, before a sibling wave committed to specs/; a33e500 alone reads 4922 and 8911,
the 75 and 127 more above are this change's spec edits, its contract-honesty corrections
included. Any count in this header is a reading, not a standing fact. Counted as lines
over specs/**/*.t27: `pub const ` at column zero, and lines whose first word is
`assert`.)

A green run also proves nothing about a fact NOBODY BOUND. That asymmetry is not
theoretical here: until 2026-09-21 catalog_write.ADMIN_SCREEN_LINES was bound to the
length of whatever file happened to be the largest under src/, so the constant could
become false about the file it is named after while the gate stayed green. It is now
two bindings, one for the length and one for the identity. The general lesson is in the
table's header: measure both sides by hand FIRST, and confirm they are the same fact.

Python 3 standard library only, for the reason scripts/verify_fleet_seed.py gives in
its own header: this repository's gates must run with no toolchain and no dependency.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# ---------------------------------------------------------------------------------
# THE TABLE
# ---------------------------------------------------------------------------------
# One entry per pair of facts that must agree. Fields:
#
#   name       short label printed on failure and under -v.
#   spec       repo-relative .t27 file.
#   const      TOP-LEVEL constant name inside it. Located by NAME, never by line
#              number: line numbers drift, and this corpus has already been burned by
#              that -- specs/turbobaby/order_status.t27:308-311 records a citation
#              whose line range "had drifted off its declarations entirely".
#   source     repo-relative .rs / .sql file the contract constrains -- or, for the
#              four tree_* extractors, a repo-relative glob. Since 2026-09-22 also a
#              tracked data file (data/fleet_seed.json, the declared SOURCE of
#              delivery_terms.t27): read_text and the tracked check never looked at the
#              extension, so a JSON source needs no new code. Also since 2026-09-22 the
#              tracked deployment manifest railway.toml, where http_cache.t27 reads its
#              replica count, on the same terms. Also since 2026-09-22 a tracked gate
#              script (scripts/verify_t27_specs.py) and a tracked .t27 that is itself the
#              published artifact a contract reasons about (specs/agents/turbobaby.t27),
#              both for publication.t27, again on the same terms.
#   extract    how to read the same fact out of `source`. One of:
#                ("regex", PATTERN)        -- one capture group, a scalar.
#                ("regex_list", PATTERN)   -- one capture group holding a list body.
#                ("regex_count", PATTERN)  -- the value IS the number of matches.
#                ("regex_all", PATTERN)    -- capture 1 of EVERY match, in file order.
#                ("regex_group_max", PAT)  -- group matches by capture 1, take the
#                                             largest group's size.
#                ("regex_group_counts",PAT)-- how many times capture 1 takes each key,
#                                             emitted in the order `order_by` gives.
#                                             Keys with no match emit 0, which is why
#                                             the order has to come from elsewhere.
#                ("line_count",)           -- the value IS the file's line count.
#                ("tree_file_count",)      -- how many files the glob matches.
#                ("tree_regex_count", PAT) -- matches of PAT across every matched file.
#                ("tree_module_count", PAT)-- how many matched files hold >= 1 match.
#                ("tree_line_nth", N)      -- line count of the Nth longest matched
#                                             file, 1-based. Answers HOW LONG the Nth
#                                             file is, and says nothing about which
#                                             file that is -- see tree_path_nth.
#                ("tree_path_nth", N)      -- repo-relative PATH of the Nth longest
#                                             matched file, for a contract whose claim
#                                             is a RANK ("the screen is the largest
#                                             file"). Split out from tree_line_nth on
#                                             2026-09-21 because binding a rank's
#                                             LENGTH to a constant named after a FILE
#                                             let the file change underneath it: with
#                                             admin_screen.rs cut to 3000 lines and two
#                                             other files padded to 5802 and 4599, the
#                                             gate reported "OK - 65 bindings hold"
#                                             while ADMIN_SCREEN_LINES = 5802 had
#                                             become false. Both extractors refuse a
#                                             TIE at the rank they are asked for: when
#                                             two files share the length, which one
#                                             holds the rank is not a fact.
#              The tree_* four exist because several contracts declare a CENSUS --
#              "84 call sites", "87 migration files" -- and a census is the kind of
#              number that rots on the next endpoint while every test stays green.
#   relation   "equal" | "list_equal" | "set_equal" | "source_at_most" |
#              "source_at_least". The bound relations read contract-side first:
#              source_at_most means source <= contract.
#   why        one line saying why the two must agree. Not decoration: a binding
#              nobody can justify is a binding that should be deleted, not muted.
#
# Optional fields:
#   occurrences   how many times a ("regex", ...) anchor is allowed to match, when
#                 the contract itself says the check is duplicated. Default 1. Every
#                 match must yield the SAME value; disagreeing copies are RED.
#   witness       a second pattern that must match at least once in `source` (or, for a
#                 tree_* binding, in at least one matched file). Required on any binding
#                 whose expected value is zero, so that a declared ABSENCE cannot be
#                 satisfied by a regex that has gone blind (D16). Evaluated by EVERY
#                 extractor: until 2026-09-21 tree_file_count and tree_line_nth returned
#                 before the check, so a witness on one of those was accepted and never
#                 run, and the D16 rule in main() could be satisfied by a dead pattern.
#                 Flags differ from the extractors': main() searches a single-file witness
#                 with re.DOTALL only and extract_tree searches a tree witness with no flags,
#                 so in NEITHER does `^` mean start-of-line -- it means start-of-file, and a
#                 witness written `^\s*#\[route` goes blind (measured 2026-09-22). Leave a
#                 witness unanchored, or spell `(?m)` into it. Since 2026-09-22 a witness is
#                 also used on a NON-zero binding, to pin a shape the value depends on and
#                 the extractor cannot see: parse_kind's refusing default arm, the unsliced
#                 loop over GENERATORS, the month band's open end.
#   resolve_in   files in which bare identifiers found inside a list may be resolved
#                 to `const IDENT: &str = "...";`. Exactly one definition must exist.
#   order_by      required by ("regex_group_counts", ...): a pattern whose capture 1,
#                 in file order, is the key sequence the counts are emitted against.
#
# To add a binding: measure both sides by hand FIRST, confirm they are the same fact
# and not two facts with similar names, then add the row. A wrong binding is worse
# than a missing one.

# One seeded unit row of migrations/082_bikes_seed.sql, in the shape
# scripts/verify_fleet_seed.py:54-63 already uses: (family, code, year, colour,
# status). Named once because four bindings below narrow it and one uses it whole as
# a witness.
STATUS_ANY = "(?:available|rented|service|retired)"
UNIT_ROW_SQL = (
    r"\('[a-z0-9\-]+',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*"
    r"'" + STATUS_ANY + r"'\)"
)
# The same row with the FAMILY KEY captured, for the per-family counts. `.replace`
# on STATUS_ANY below narrows it to one status without restating the row shape.
UNIT_ROW_KEYED_SQL = (
    r"\('([a-z0-9\-]+)',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*"
    r"'" + STATUS_ANY + r"'\)"
)
# One Phuket zone row of migrations/087_delivery_zones_phuket.sql's VALUES list --
# (name, name_en, fee, min_order, sort_order) -- and the same row of src/delivery.rs's
# PHUKET_ZONES -- (id, name, name_en, fee). The _NAME forms capture name_en. The pickup row is
# neither (087 does not insert it, and src/delivery.rs declares it apart as PICKUP_ZONE), and
# the two-element rows of delivery.rs's own test table do not match either shape.
PHUKET_ROW_SQL = r"^[ \t]+\('[^'\n]+',[ \t]*'[^'\n]+',[ \t]*[0-9.]+,[ \t]*[0-9.]+,[ \t]*\d+\),?[ \t]*$"
PHUKET_ROW_SQL_NAME = r"^[ \t]+\('[^'\n]+',[ \t]*'([^'\n]+)',[ \t]*[0-9.]+,[ \t]*[0-9.]+,[ \t]*\d+\),?[ \t]*$"
PHUKET_ROW_RS = r'^[ \t]+\("[a-z_]+",[ \t]*"[^"\n]+",[ \t]*"[^"\n]+",[ \t]*[0-9.]+\),[ \t]*$'
PHUKET_ROW_RS_NAME = r'^[ \t]+\("[a-z_]+",[ \t]*"[^"\n]+",[ \t]*"([^"\n]+)",[ \t]*[0-9.]+\),[ \t]*$'
# One families-block row, key captured. migrations/082 seeds families before units, so
# this is also the order availability.t27's four parallel arrays are written in.
FAMILY_ROW_SQL = (
    r"\('([a-z0-9\-]+)',\s*'[^']*',\s*'[^']*',\s*(?:NULL|'[^']*'),\s*"
    r"'(?:scooter|motorcycle)',"
)

# The prefix that keeps a Rust count from being satisfied by commented-out code. Added
# 2026-09-22 on review: seven count rows below matched a gate or a call on a line that had
# been commented out -- the likeliest way to switch a one-line gate off -- and stayed green
# (planted, each one). Both extractor paths compile with re.MULTILINE, so `^` is a line
# start. The lookahead refuses a line whose first non-blank characters are `//` (which covers
# `///` and `//!`), and the run after it may not cross a `//`, so a hit inside a trailing
# `// ...` comment is refused too. What it costs, and what it still does not see:
#   - a line counts AT MOST ONCE: every match starts at a line start, so two hits on one
#     line read as one. Measured 2026-09-22: every count that uses it is the same with and
#     without it, so no line holds two hits and no hit sits in a comment today;
#   - a `//` inside a string literal earlier on the same line (a URL) hides the hit: an
#     under-count, red, the fail-closed direction;
#   - a /* ... */ block comment is NOT excluded, and neither is text inside a string.
# Not used on a count of method handlers: `get(a).post(b)` on one line is the chained route
# those rows exist to see, and this prefix would count it once (see METHOD_HANDLER).
NOT_IN_A_LINE_COMMENT = r"^(?![ \t]*//)(?:[^\n/]|/(?!/))*?"

# One method router of an axum route table -- `get(h)`, `post(h)`, ... -- standing alone, or
# chained after `)`, or at a line start after whitespace. Counts HANDLERS, so a `.patch(h)`
# chained onto an existing registration counts where a `.route(` count does not.
# `headers.get("..")` and `map.get(k)` do not match. Shared by the five route counts below
# since 2026-09-22 (the loyalty and referral rows spelled it first). Not comment-aware: a
# route line commented out in place still counts, so the count over-states the surface --
# the direction in which nothing new is exposed -- and a route commented out AND an ungated
# one added in the same change keep the total. NOT_IN_A_LINE_COMMENT cannot close that here
# without making a same-line chain count once.
METHOD_HANDLER = r"(?:(?<![\w.])|(?<=\)\.)|(?<=\s\.))(?:get|post|put|patch|delete)\([\w:]+\)"

BINDINGS: tuple[dict[str, object], ...] = (
    # --- unit status vocabulary, three ways ---------------------------------------
    # specs/turbobaby/availability.t27:127 already names the shipped witnesses:
    # src/api/bikes.rs:794 and src/api/bikes.rs:797-803. The array there is built out
    # of const references, one of which lives in another module, so the identifiers
    # are resolved rather than matched as text.
    {
        "name": "availability.STATUS_NAMES ~ bikes.rs UNIT_STATUSES",
        "spec": "specs/turbobaby/availability.t27",
        "const": "STATUS_NAMES",
        "source": "src/api/bikes.rs",
        "extract": ("regex_list", r"const UNIT_STATUSES:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\]\s*;"),
        "resolve_in": ("src/api/bikes.rs", "src/db/bikes.rs"),
        "relation": "list_equal",
        "why": "the API pre-seeds by_status from this array, so a status the contract "
               "names and the array omits is published to the admin screen as absent "
               "rather than as zero",
    },
    {
        "name": "availability.STATUS_NAMES ~ 078 status CHECK",
        "spec": "specs/turbobaby/availability.t27",
        "const": "STATUS_NAMES",
        "source": "migrations/078_bike_units.sql",
        "extract": ("regex_list", r"CHECK\s*\(status IN \((.*?)\)\)"),
        "relation": "set_equal",
        "why": "the column CHECK is what a write actually has to pass; the contract "
               "names the same four and cites this line (availability.t27:111)",
    },
    # --- order status vocabulary ---------------------------------------------------
    {
        "name": "order_status.STATUS_NAMES ~ orders.rs VALID_STATUSES",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "STATUS_NAMES",
        "source": "src/api/orders.rs",
        "extract": ("regex_list", r"const VALID_STATUSES:\s*&\[&str\]\s*=\s*&\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "the nine names are the whole admission test for PATCH order status; a "
               "name in one list and not the other is either a 400 on a legal status "
               "or a contract promising a status the endpoint refuses",
    },
    {
        "name": "order_status.TERMINAL_NAMES ~ orders.rs is_terminal arm",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "TERMINAL_NAMES",
        "source": "src/api/orders.rs",
        "extract": ("regex_list", r"let is_terminal = matches!\(\s*cur_status,\s*(.*?)\s*\)\s*;"),
        "relation": "list_equal",
        "why": "terminality decides whether a status change is refused; the contract "
               "states these four ARE the source's own order (order_status.t27:224)",
    },
    {
        "name": "order_status.STATUS_CHARS_MAX ~ orders.rs status length guard",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "STATUS_CHARS_MAX",
        "source": "src/api/orders.rs",
        "extract": ("regex", r"if status\.len\(\) > (\d+)\s*\{"),
        "relation": "equal",
        "why": "the cap is checked before the whitelist, so it is the first thing a "
               "malformed status meets and the contract pins its precedence",
    },
    {
        "name": "order_status.ORDER_ID_CHARS_MAX ~ orders.rs id length guard",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "ORDER_ID_CHARS_MAX",
        "source": "src/api/orders.rs",
        # `id.len() > 200` alone matches 8 times in this file. The function signature
        # is the anchor that makes it one.
        "extract": ("regex", r"fn validate_update_order_status\([^)]*\)[^{]*\{\s*if id\.len\(\) > (\d+)"),
        "relation": "equal",
        "why": "same refusal ladder as STATUS_CHARS_MAX; the contract pins both bounds "
               "and their order (order_status.t27:298-302)",
    },
    # --- star award ----------------------------------------------------------------
    {
        "name": "star_award.CAPPED_SOURCES ~ stars_cap.rs GAME_SOURCES",
        "spec": "specs/turbobaby/star_award.t27",
        "const": "CAPPED_SOURCES",
        "source": "src/trios/stars_cap.rs",
        "extract": ("regex_list", r"pub const GAME_SOURCES:\s*&\[&str\]\s*=\s*&\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "a spelling dropped from the source list uncaps a replay of that "
               "source, which is the exploit stars_cap.rs:16-18 exists to stop; Stars "
               "are 1:1 baht at checkout, so an uncapped source is an open till",
    },
    {
        "name": "star_award.PER_SOURCE_WINDOW_CAP ~ stars_cap.rs DAILY_GAME_STARS_CAP",
        "spec": "specs/turbobaby/star_award.t27",
        "const": "PER_SOURCE_WINDOW_CAP",
        "source": "src/trios/stars_cap.rs",
        "extract": ("regex", r"pub const DAILY_GAME_STARS_CAP:\s*i64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the ceiling on what one game source may pay one player per window; the "
               "contract's REACHABLE_WINDOW_TOTAL_STARS is four times this number, so "
               "a drift here silently moves the shop's whole exposure",
    },
    {
        "name": "star_award.MAX_STARS_PER_CALL ~ stars.rs MAX_STARS_PER_TX",
        "spec": "specs/turbobaby/star_award.t27",
        "const": "MAX_STARS_PER_CALL",
        "source": "src/api/stars.rs",
        "extract": ("regex", r"const MAX_STARS_PER_TX:\s*i64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the per-call ceiling the endpoint enforces before the cap trio is "
               "consulted at all; star_award.t27:198 declares it as the outer bound",
    },
    # --- order money ----------------------------------------------------------------
    {
        "name": "order_money.MAX_STARS_PER_TRANSACTION ~ orders.rs MAX_STARS_PER_TX",
        "spec": "specs/turbobaby/order_money.t27",
        "const": "MAX_STARS_PER_TRANSACTION",
        "source": "src/api/orders.rs",
        # A SECOND constant of the same name lives at src/api/stars.rs:52. They are
        # different constants on different paths -- spend vs award -- and the contract
        # pins the spend one by file and line (order_money.t27:124).
        "extract": ("regex", r"const MAX_STARS_PER_TX:\s*i64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "how many Stars one checkout may spend; at 1 Star = 1 baht this is a "
               "discount ceiling, and the contract is the only place the two "
               "same-named ceilings are told apart",
    },
    {
        "name": "order_money.MAX_ORDER_AMOUNT_MAJOR_UNITS ~ orders.rs MAX_ORDER_TOTAL",
        "spec": "specs/turbobaby/order_money.t27",
        "const": "MAX_ORDER_AMOUNT_MAJOR_UNITS",
        "source": "src/api/orders.rs",
        "extract": ("regex", r"const MAX_ORDER_TOTAL:\s*f64\s*=\s*([0-9_.]+)\s*;"),
        "relation": "equal",
        "why": "the envelope both subtotal and total must sit inside; the contract's "
               "verdict algebra is written against exactly this figure",
    },
    # --- catalog write bounds ---------------------------------------------------------
    {
        "name": "catalog_write.WRITE_DISPLACEMENT_MAX_CC ~ bikes.rs MAX_DISPLACEMENT_CC",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "WRITE_DISPLACEMENT_MAX_CC",
        "source": "src/api/bikes.rs",
        "extract": ("regex", r"const MAX_DISPLACEMENT_CC:\s*i32\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "catalog_write.t27 records that the COLUMN has no displacement ceiling "
               "(COLUMN_HAS_A_DISPLACEMENT_CEILING = false), so this API constant is the "
               "only one there is; the contract is the only statement of what it is",
    },
    {
        "name": "catalog_write.WRITE_TEXT_MAX_CHARS ~ bikes.rs MAX_TEXT_LEN",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "WRITE_TEXT_MAX_CHARS",
        "source": "src/api/bikes.rs",
        "extract": ("regex", r"const MAX_TEXT_LEN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the ceiling on both description fields; the contract also fixes that "
               "it counts characters and not bytes, which only means something while "
               "the number itself is right",
    },
    {
        "name": "catalog_write.WRITE_KEY_MAX_CHARS ~ bikes.rs MAX_KEY_LEN",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "WRITE_KEY_MAX_CHARS",
        "source": "src/api/bikes.rs",
        "extract": ("regex", r"const MAX_KEY_LEN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "same constant guards the lookup path segment and the admin write, so "
               "one number governs a read and a write the contract describes apart",
    },
    {
        "name": "catalog_write.WRITE_UNIT_CODE_MAX_CHARS ~ bikes.rs unit code guard",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "WRITE_UNIT_CODE_MAX_CHARS",
        "source": "src/api/bikes.rs",
        "extract": ("regex", r"if code\.chars\(\)\.count\(\) > (\d+)\s*\{"),
        "relation": "equal",
        "why": "an inline ceiling with no name of its own -- catalog_write.t27 counts "
               "seven such sites (INLINE_TEXT_CEILING_SITE_COUNT), and the contract is "
               "where they are written down at all",
    },
    # --- class vocabulary, three ways --------------------------------------------------
    {
        "name": "bike_catalog.CLASSES ~ bikes.rs BIKE_CLASSES",
        "spec": "specs/turbobaby/bike_catalog.t27",
        "const": "CLASSES",
        "source": "src/api/bikes.rs",
        "extract": ("regex_list", r"const BIKE_CLASSES:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "the class discriminant is the INDEX into this array (bike_catalog.t27:"
               "111), so reordering it renumbers every class code in the corpus",
    },
    {
        "name": "bike_catalog.CLASSES ~ 077 class CHECK",
        "spec": "specs/turbobaby/bike_catalog.t27",
        "const": "CLASSES",
        "source": "migrations/077_bikes.sql",
        "extract": ("regex_list", r"class TEXT NOT NULL CHECK \(class IN \((.*?)\)\)"),
        "relation": "set_equal",
        "why": "the column decides which class discount joins in 079; the API list is "
               "explicitly a duplicate of this one (src/api/bikes.rs:938-943)",
    },
    # --- game score --------------------------------------------------------------------
    {
        "name": "game_score.SCORE_MAX_ACCEPTED ~ game.rs MAX_SCORE",
        "spec": "specs/turbobaby/game_score.t27",
        "const": "SCORE_MAX_ACCEPTED",
        "source": "src/api/game.rs",
        "extract": ("regex", r"const MAX_SCORE:\s*i64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the leaderboard admits anything at or under this, and the contract's "
               "whole admission algebra is written against the figure",
    },
    {
        "name": "game_score.DISPLAY_NAME_MAX_LEN ~ game.rs MAX_DISPLAY_NAME_LEN",
        "spec": "specs/turbobaby/game_score.t27",
        "const": "DISPLAY_NAME_MAX_LEN",
        "source": "src/api/game.rs",
        "extract": ("regex", r"const MAX_DISPLAY_NAME_LEN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "a public board renders this string; the cap is the only thing between "
               "a player-chosen name and the width of the screen",
    },
    {
        "name": "game_score.IDENTIFIER_DIGIT_RUN ~ game.rs IDENTIFIER_DIGIT_RUN",
        "spec": "specs/turbobaby/game_score.t27",
        "const": "IDENTIFIER_DIGIT_RUN",
        "source": "src/api/game.rs",
        "extract": ("regex", r"const IDENTIFIER_DIGIT_RUN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the anonymous board masks a name holding this many digits in a row even when "
               "the row's own Telegram id cannot be read (2026-09-24 leak fix)",
    },
    # --- request identity -----------------------------------------------------------------
    {
        "name": "request_identity.INIT_DATA_MAX_BYTES ~ auth.rs size cap",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "INIT_DATA_MAX_BYTES",
        "source": "src/api/auth.rs",
        "extract": ("regex", r"if init_data\.len\(\) > (\d+)\s*\{"),
        "relation": "equal",
        "why": "the cap runs BEFORE parsing (request_identity.t27:240), so it is the "
               "bound on what an unauthenticated caller can make the parser do",
    },
    {
        "name": "request_identity.AUTH_DATE_MAX_AGE_SECONDS ~ auth.rs freshness",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "AUTH_DATE_MAX_AGE_SECONDS",
        "source": "src/api/auth.rs",
        "extract": ("regex", r"now\.saturating_sub\(\w+\) > (\d+)"),
        # FRESHNESS_IS_CHECKED_IN_BOTH_PATHS (request_identity.t27:259) says two, so
        # two is what this binding demands -- and both must carry the same number.
        "occurrences": 2,
        "relation": "equal",
        "why": "how long a replayed initData stays usable, in the strict validator and "
               "the lenient owner path; the contract asserts BOTH paths check it, so a "
               "third path or a missing one is the interesting failure",
    },
    {
        "name": "request_identity.ADMIN_RL_MAX_ATTEMPTS ~ auth.rs attempt cap",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "ADMIN_RL_MAX_ATTEMPTS",
        "source": "src/api/auth.rs",
        "extract": ("regex", r"const ADMIN_AUTH_RL_MAX_ATTEMPTS:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "how many admin-password guesses one IP gets per window; the only "
               "number standing between a leaked endpoint and an offline-speed attack",
    },
    {
        "name": "request_identity.ADMIN_RL_WINDOW_SECONDS ~ auth.rs window",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "ADMIN_RL_WINDOW_SECONDS",
        "source": "src/api/auth.rs",
        # Written as a product (`5 * 60`); the reader evaluates products of integer
        # literals rather than demanding the source spell out 300.
        "extract": ("regex", r"const ADMIN_AUTH_RL_WINDOW:[^=]*=\s*[\w:]*Duration::from_secs\(([^)]+)\)"),
        "relation": "equal",
        "why": "the other half of the attempt cap -- ten attempts means nothing "
               "without the window they are counted in",
    },
    # --- rate limit -----------------------------------------------------------------------
    {
        "name": "rate_limit.EXAMPLE_WINDOW_SECONDS ~ upload.rs UPLOAD_RL_WINDOW",
        "spec": "specs/turbobaby/rate_limit.t27",
        "const": "EXAMPLE_WINDOW_SECONDS",
        "source": "src/api/upload.rs",
        "extract": ("regex", r"const UPLOAD_RL_WINDOW:\s*Duration\s*=\s*Duration::from_secs\(([^)]+)\)"),
        "relation": "equal",
        "why": "rate_limit.t27:336-340 names POST /upload as its one exemplar and "
               "cites this very line; an exemplar that has stopped being true is worse "
               "than none, because the contract offers it as the reader's calibration",
    },
    {
        "name": "rate_limit.EXAMPLE_MAX_ATTEMPTS ~ upload.rs UPLOAD_RL_MAX_ATTEMPTS",
        "spec": "specs/turbobaby/rate_limit.t27",
        "const": "EXAMPLE_MAX_ATTEMPTS",
        "source": "src/api/upload.rs",
        "extract": ("regex", r"const UPLOAD_RL_MAX_ATTEMPTS:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "same exemplar, the attempt half of it",
    },
    {
        "name": "rate_limit.HEADER_VALUE_MAX_BYTES ~ rate_limit.rs ip length guard",
        "spec": "specs/turbobaby/rate_limit.t27",
        "const": "HEADER_VALUE_MAX_BYTES",
        "source": "src/api/rate_limit.rs",
        "extract": ("regex", r"ip\.len\(\) <= (\d+)"),
        # x-forwarded-for and x-real-ip each carry the same bound.
        "occurrences": 2,
        "relation": "equal",
        "why": "a header longer than this falls through to the shared bucket, which "
               "the contract calls reachable on purpose; the number decides how easily",
    },
    {
        "name": "rate_limit.SOURCE_LINES ~ rate_limit.rs length",
        "spec": "specs/turbobaby/rate_limit.t27",
        "const": "SOURCE_LINES",
        "source": "src/api/rate_limit.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "rate_limit.t27 declares the size of the file it describes, and the "
               "rest of that contract is a census of it -- a changed file means the "
               "census was taken against a different one",
    },
    # --- upload media -----------------------------------------------------------------------
    {
        "name": "upload_media.ACCEPTED_EXTENSIONS ~ upload.rs ALLOWED_EXTENSIONS",
        "spec": "specs/turbobaby/upload_media.t27",
        "const": "ACCEPTED_EXTENSIONS",
        "source": "src/api/upload.rs",
        "extract": ("regex_list", r"const ALLOWED_EXTENSIONS:\s*&\[&str\]\s*=\s*&\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "the allowlist keys on the EXTENSION and nothing reads the declared "
               "content type (upload_media.t27:274-275), so this array is the entire "
               "admission decision for an uploaded file",
    },
    {
        "name": "upload_media.HANDLER_MAX_BYTES ~ upload.rs MAX_UPLOAD_SIZE",
        "spec": "specs/turbobaby/upload_media.t27",
        "const": "HANDLER_MAX_BYTES",
        "source": "src/api/upload.rs",
        "extract": ("regex", r"const MAX_UPLOAD_SIZE:\s*usize\s*=\s*([0-9_ *]+);"),
        "relation": "equal",
        "why": "the handler's own ceiling, enforced per chunk; the contract's claim "
               "that the second size check is unreachable depends on this figure",
    },
    {
        "name": "upload_media.LAYER_MAX_BYTES ~ upload.rs DefaultBodyLimit",
        "spec": "specs/turbobaby/upload_media.t27",
        "const": "LAYER_MAX_BYTES",
        "source": "src/api/upload.rs",
        "extract": ("regex", r"DefaultBodyLimit::max\(([^)]+)\)"),
        "relation": "equal",
        "why": "the route escapes the general body limit with its own; the headroom "
               "between this and HANDLER_MAX_BYTES is the multipart framing, and the "
               "contract states that gap as a number",
    },
    # --- locale policy ---------------------------------------------------------------------
    {
        "name": "locale_policy.PUBLISHED_LOCALES ~ locales.rs supported_langs",
        "spec": "specs/turbobaby/locale_policy.t27",
        "const": "PUBLISHED_LOCALES",
        "source": "src/locales.rs",
        "extract": ("regex_list", r"fn supported_langs\(\)[^{]*\{\s*vec!\[(.*?)\]\s*\}"),
        "relation": "list_equal",
        "why": "locale_policy.t27:107 names this function as the published set's "
               "source, and it becomes one picker button per element in the bot",
    },
    # --- the fleet table availability.t27 says is ungated ---------------------------------------
    # availability.t27:116-119 and :140-142 both record that scripts/verify_fleet_seed.py
    # gates turbobaby/market and nothing else, that the word "availability" does not
    # occur in it, and that "comparing this table to the seed is still the next gate to
    # write". These four bindings are that gate, taken against the SQL that actually
    # reaches the database rather than against the JSON -- verify_fleet_seed.py already
    # ties the JSON to the same SQL, so the chain closes without a second opinion.
    {
        "name": "availability.FLEET_UNITS ~ 082 unit rows",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FLEET_UNITS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", UNIT_ROW_SQL),
        "relation": "equal",
        "why": "the seeded unit count is the fleet the contract publishes; every "
               "per-family figure in the contract has to add up to it",
    },
    {
        "name": "availability.FLEET_RENTED ~ 082 rented rows",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FLEET_RENTED",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", r"\('[a-z0-9\-]+',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*'rented'\)"),
        "relation": "equal",
        "why": "rented units are the ones a customer cannot be handed; quoting one is "
               "the failure D9 and D12 exist to prevent",
    },
    {
        "name": "availability.FLEET_AVAILABLE ~ 082 available rows",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FLEET_AVAILABLE",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", r"\('[a-z0-9\-]+',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*'available'\)"),
        "relation": "equal",
        "why": "the number the contract says available_count matches exactly",
    },
    {
        "name": "availability.FAMILIES_IN_STOCK ~ 082 family rows",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FAMILIES_IN_STOCK",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", r"\('[a-z0-9\-]+',\s*'[^']*',\s*'[^']*',\s*(?:NULL|'[^']*'),\s*'(?:scooter|motorcycle)',"),
        "relation": "equal",
        "why": "a family seeded into `bikes` is one a customer can be shown; the "
               "contract's FAMILY_KEYS array has to be exactly this long",
    },
    {
        "name": "availability.MEASURED_SERVICE_OR_RETIRED_UNITS ~ 082 absence",
        "spec": "specs/turbobaby/availability.t27",
        "const": "MEASURED_SERVICE_OR_RETIRED_UNITS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", r"\('[a-z0-9\-]+',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*'(?:service|retired)'\)"),
        # The expected value is ZERO, so a blind regex would satisfy this binding by
        # matching nothing at all -- exactly the D16 failure. The witness proves the
        # row shape is still being recognised in this file before the zero is believed.
        "witness": UNIT_ROW_SQL,
        "relation": "equal",
        "why": "37 - 11 - 26 = 0 only holds while no seeded unit is in service or "
               "retired; the contract declares that absence as a measurement",
    },
    {
        "name": "availability.FAMILY_KEYS ~ 082 family rows",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FAMILY_KEYS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_all", FAMILY_ROW_SQL),
        "relation": "list_equal",
        "why": "the three parallel arrays below are indexed by position in this one, "
               "so a family inserted anywhere but the end silently re-labels every "
               "unit count after it",
    },
    {
        "name": "availability.FAMILY_UNITS ~ 082 units per family",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FAMILY_UNITS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_group_counts", UNIT_ROW_KEYED_SQL),
        "order_by": FAMILY_ROW_SQL,
        "relation": "list_equal",
        "why": "availability.t27:140-142 names exactly this comparison as the gate "
               "still to write, and says the word 'availability' does not occur in "
               "scripts/verify_fleet_seed.py at all",
    },
    {
        "name": "availability.FAMILY_RENTED ~ 082 rented per family",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FAMILY_RENTED",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_group_counts", UNIT_ROW_KEYED_SQL.replace(STATUS_ANY, "rented")),
        "order_by": FAMILY_ROW_SQL,
        "relation": "list_equal",
        "why": "six of the fourteen families have zero rented units, so the count has "
               "to be emitted against the key order rather than against the rows that "
               "happen to exist -- a shorter list would line up wrong and stay green",
    },
    {
        "name": "availability.FAMILY_AVAILABLE ~ 082 available per family",
        "spec": "specs/turbobaby/availability.t27",
        "const": "FAMILY_AVAILABLE",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_group_counts", UNIT_ROW_KEYED_SQL.replace(STATUS_ANY, "available")),
        "order_by": FAMILY_ROW_SQL,
        "relation": "list_equal",
        "why": "this is the column a customer sees; a family quoted as bookable with "
               "zero available units is the failure D9 and D12 name",
    },
    {
        "name": "availability.MAX_UNITS_PER_FAMILY ~ 082 largest family",
        "spec": "specs/turbobaby/availability.t27",
        "const": "MAX_UNITS_PER_FAMILY",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_group_max", r"\('([a-z0-9\-]+)',\s*'[a-z0-9\-]+',\s*(?:NULL|\d+),\s*(?:NULL|'[^']*'),\s*'(?:available|rented|service|retired)'\)"),
        "relation": "source_at_most",
        "why": "the contract's UnitSlot arithmetic is written over fixed [10]u8 slot "
               "arrays and truncates at MAX_UNITS_PER_FAMILY; a family seeded with "
               "more units than that is counted wrong by the contract's own functions",
    },
    # --- what the customer screens say about availability (2026-09-24) ---------------------------
    # availability.t27 records two corrections of that date, and each is a fact about the UI text
    # that a later edit could silently undo. Both measured by hand first: the Rust constant held
    # ["PCX 150", "ADV 150", "NMAX 155"] and now holds ["NMAX 155"]; the confirming keys were used
    # on 7 code lines under src/ui/ (5 in catalog_screen.rs, 2 in bike_detail.rs) and now on 0.
    {
        "name": "availability.CLICK_125_REDIRECT_LABELS_SHOWN ~ catalog_screen.rs CLICK_125_ALTERNATIVES",
        "spec": "specs/turbobaby/availability.t27",
        "const": "CLICK_125_REDIRECT_LABELS_SHOWN",
        "source": "src/ui/screens/catalog_screen.rs",
        "extract": ("regex_list", r"const CLICK_125_ALTERNATIVES:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "the closed CLICK 125 card prints this list as where to go instead; a label the "
               "contract does not record is a model the owner's rules forbid naming even as a "
               "replacement, and tests/catalog_honesty_wiring.rs ties each label to the seed",
    },
    # The owner's decision of 2026-09-25 ("for now", NMAX 155 alone) is recorded twice: in the
    # contract as CLICK_125_REDIRECTS / CLICK_125_REDIRECTS_ARE_PROVISIONAL and in the seed as
    # pricing_policy.not_offered.click-125 offer_instead / offer_instead_provisional. Measured by
    # hand on both sides first: ["nmax-155"] and true in each; until that date the two lists read
    # ["pcx-150", "adv-150", "nmax-155"] in both files and nothing tied them. The anchors are keyed
    # on the quoted field name with its closing quote, so the seed's offer_instead_before_2026_09_25
    # and offer_instead_source are not read (each anchor must match exactly once).
    {
        "name": "availability.CLICK_125_REDIRECTS ~ seed click-125 offer_instead",
        "spec": "specs/turbobaby/availability.t27",
        "const": "CLICK_125_REDIRECTS",
        "source": "data/fleet_seed.json",
        "extract": ("regex_list", r'"offer_instead":\s*\[(.*?)\]'),
        "relation": "list_equal",
        "why": "the seed and the contract each record what the owner decided to offer instead of "
               "CLICK 125; a list re-decided in one and not the other leaves the repository "
               "offering two things, and the screen's label is tied to the contract, not the seed",
    },
    {
        "name": "availability.CLICK_125_REDIRECTS_ARE_PROVISIONAL ~ seed offer_instead_provisional",
        "spec": "specs/turbobaby/availability.t27",
        "const": "CLICK_125_REDIRECTS_ARE_PROVISIONAL",
        "source": "data/fleet_seed.json",
        "extract": ("regex", r'"offer_instead_provisional":\s*(true|false)\s*[,}]'),
        "relation": "equal",
        "why": "the owner said \"for now\"; a decision made final (or re-opened) in the seed "
               "under a contract that still calls it provisional, or the reverse, misstates "
               "how settled the redirect is",
    },
    {
        "name": "availability.CONFIRMING_KEY_LINES_IN_UI ~ src/ui/ absence",
        "spec": "specs/turbobaby/availability.t27",
        "const": "CONFIRMING_KEY_LINES_IN_UI",
        "source": "src/ui/**/*.rs",
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT
                    + r"\bT_BIKE_(?:AVAILABILITY|AVAILABILITY_FREE|COLORS_AVAILABLE|FILTER_FREE_NOW)\b"),
        # The expected value is ZERO, so the scan must be shown to see the key that replaced
        # them before the zero is believed (D16).
        "witness": r"T_BIKE_AVAILABILITY_UNKNOWN",
        "relation": "equal",
        "why": "FILE_MAY_CONFIRM is false: each of the four keys prints the seeded, admin-edited "
               "unit count to a customer as if a live check had confirmed it, and a line that "
               "uses one again brings that promise back",
    },
    # --- censuses: the numbers that rot on the next endpoint -------------------------------------
    # request_identity.t27:312-318 declares how many places each authorisation gate is
    # called from. The lookbehind drops the one `fn check_admin(` that is the
    # definition; without it every count is one too high.
    {
        "name": "request_identity.GATE_CALL_SITES_ADMIN ~ src/ census",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "GATE_CALL_SITES_ADMIN",
        "source": "src/**/*.rs",
        "extract": ("tree_regex_count", r"(?<!fn )check_admin\("),
        "relation": "equal",
        # The `why` here used to read "a new admin route that forgets the call moves
        # this number and nothing else notices". That is FALSE in the direction that
        # matters: a route which omits check_admin adds ZERO occurrences, so the census
        # cannot see it. Only a route that DOES call the gate moves the count. The
        # contract makes no such claim either -- request_identity.t27:307-310 says the
        # counts exist "to say why no endpoint-to-gate table lives here", and :319
        # declares ENDPOINT_TO_GATE_MAP_IS_NOT_OWNED_HERE = true. A binding justified by
        # a sentence stronger than what it checks is worse than an unjustified one.
        "why": "the declared size of the admin gate's call surface. It catches the "
               "count going stale -- a site added or removed without re-measuring the "
               "contract -- and NOT an unguarded endpoint, which adds no occurrence at "
               "all and is outside what request_identity.t27:319 claims to own",
    },
    {
        "name": "request_identity.GATE_CALL_SITES_OWNER ~ src/ census",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "GATE_CALL_SITES_OWNER",
        "source": "src/**/*.rs",
        "extract": ("tree_regex_count", r"(?<!fn )check_owner\("),
        "relation": "equal",
        "why": "same census for the strict owner gate",
    },
    {
        "name": "request_identity.GATE_CALL_SITES_OWNER_LENIENT ~ src/ census",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "GATE_CALL_SITES_OWNER_LENIENT",
        "source": "src/**/*.rs",
        "extract": ("tree_regex_count", r"(?<!fn )check_owner_lenient\("),
        "relation": "equal",
        "why": "the lenient gate is the weakest of the three, so its spread is the one "
               "worth knowing has not grown",
    },
    {
        "name": "request_identity.GATE_CALL_SITES_TOTAL ~ src/ census",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "GATE_CALL_SITES_TOTAL",
        "source": "src/**/*.rs",
        "extract": ("tree_regex_count", r"(?<!fn )check_(?:admin|owner_lenient|owner)\("),
        "relation": "equal",
        # Also corrected 2026-09-21. The old `why` said this row "catches a fourth gate
        # nobody declared", and it cannot: the pattern enumerates the same three names
        # the rows above do, so a `check_moderator(` would be invisible to it. What it
        # does catch is the contract's own arithmetic -- a declared TOTAL that is not
        # the sum of the three declared parts. That is worth a row; the other claim was
        # not true.
        "why": "the contract declares the total as well as the three parts, and this "
               "row is what keeps the declared total equal to their sum; it enumerates "
               "the same three names, so a FOURTH gate would be invisible to it and to "
               "the three rows above",
    },
    {
        "name": "request_identity.GATE_CALL_SITE_MODULES ~ src/ census",
        "spec": "specs/turbobaby/request_identity.t27",
        "const": "GATE_CALL_SITE_MODULES",
        "source": "src/**/*.rs",
        "extract": ("tree_module_count", r"(?<!fn )check_(?:admin|owner_lenient|owner)\("),
        "relation": "equal",
        "why": "how wide the gate surface is, not just how deep; a module that starts "
               "authorising is a module that has joined this contract's subject",
    },
    {
        "name": "rate_limit.PEER_ADDRESS_OCCURRENCES_IN_SRC ~ src/ absence",
        "spec": "specs/turbobaby/rate_limit.t27",
        "const": "PEER_ADDRESS_OCCURRENCES_IN_SRC",
        "source": "src/**/*.rs",
        "extract": ("tree_regex_count", r"peer_addr"),
        # Expected value is zero, so the scan must be shown to be able to see anything
        # at all before the zero counts as a measurement (D16).
        "witness": r"fn \w+\(",
        "relation": "equal",
        "why": "rate_limit.t27:300-302 rests its whole proxy-trust paragraph on nothing "
               "reading the TCP peer address; the day one line does, the limiter key "
               "stops being caller-chosen and that paragraph becomes wrong",
    },
    # The migration-file census. CORRECTED 2026-09-24: this comment said that pricing_honesty.t27
    # OWNS the count and locale_policy.t27 carries a named copy (and cited both by line). A third
    # file, schema_provenance.t27, declared itself the owner as well, and it was the one of the
    # three whose figures no row bound. Resolved by subject: the census is turbobaby/schema-
    # provenance's (SQL_FILES_RECURSIVE, SQL_FILES_TOP_LEVEL, WIP_SQL_FILES, bound in the three rows
    # after this group), and the pricing-honesty and locale-policy figures are declared copies that
    # name it. Every declaration of the count is bound to the same directory, so no copy can move
    # without its owner or without the tree.
    {
        "name": "pricing_honesty.MIGRATION_FILES_SEARCHED ~ migrations/ recursive",
        "spec": "specs/turbobaby/pricing_honesty.t27",
        "const": "MIGRATION_FILES_SEARCHED",
        "source": "migrations/**/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the scope of every 'no migration constrains this' claim in the corpus; "
               "a claim about a search is worth what the search covered",
    },
    {
        "name": "locale_policy.MIGRATION_FILES_SEARCHED ~ migrations/ recursive",
        "spec": "specs/turbobaby/locale_policy.t27",
        "const": "MIGRATION_FILES_SEARCHED",
        "source": "migrations/**/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the declared copy of the owner's figure; D15 drift is two contracts "
               "holding one measurement, and this is the check that keeps them equal",
    },
    {
        "name": "pricing_honesty.MIGRATION_COUNT_TOP_LEVEL ~ migrations/*.sql",
        "spec": "specs/turbobaby/pricing_honesty.t27",
        "const": "MIGRATION_COUNT_TOP_LEVEL",
        "source": "migrations/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the contract splits the recursive count into the top level plus the wip file "
               "precisely because a top-level-only count would have been one short while "
               "claiming to have looked everywhere",
    },
    {
        "name": "pricing_honesty.MIGRATION_COUNT_IN_WIP ~ migrations/wip/*.sql",
        "spec": "specs/turbobaby/pricing_honesty.t27",
        "const": "MIGRATION_COUNT_IN_WIP",
        "source": "migrations/wip/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the other half of that split; it is the half a recursive search adds "
               "and a careless one drops",
    },
    # Added 2026-09-24: the census owner's own three figures, measured the same way as the copies
    # above. Planted RED through a mirror of the tracked tree with one .sql file added under
    # migrations/wip/ (RECURSIVE and WIP) and one at the top level (RECURSIVE and TOP_LEVEL).
    {
        "name": "schema_provenance.SQL_FILES_RECURSIVE ~ migrations/ recursive",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "SQL_FILES_RECURSIVE",
        "source": "migrations/**/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the census owner's count of every .sql file a recursive reader sees; the two "
               "copies of it in pricing-honesty and locale-policy were bound and the owner's "
               "was not, which left the self-declared owner the one unguarded declaration",
    },
    {
        "name": "schema_provenance.SQL_FILES_TOP_LEVEL ~ migrations/*.sql",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "SQL_FILES_TOP_LEVEL",
        "source": "migrations/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "what the three non-recursive read_dir walks see; the contract's collision "
               "tripwire rests on the gap between this and the recursive count",
    },
    {
        "name": "schema_provenance.WIP_SQL_FILES ~ migrations/wip/*.sql",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "WIP_SQL_FILES",
        "source": "migrations/wip/*.sql",
        "extract": ("tree_file_count",),
        "relation": "equal",
        "why": "the one file only a recursive reader sees, and the one whose number (077) "
               "collides with a shipped migration the day it is promoted without a renumber",
    },
    # nmax-155's published day rate. Added 2026-09-24 with the F4 resolution: pricing-honesty
    # owns the figure (CHEAPEST_IN_STOCK_DAY_RATE_THB, the census reading "cheapest rate on a family
    # with units") and rental-terms carries it as REF_BASE_THB_DAY, a declared copy it computes its
    # audit arithmetic on. Until then neither declaration was bound. Both rows read the same seed
    # row by its family key, so the owner, the copy and the seed cannot disagree in silence. The
    # rows do NOT prove the census half of the owner's claim (that no in-stock family is cheaper):
    # that is arithmetic over twenty rows, and it stays the contract's own assertion. Planted RED
    # (both rows) through --source-override on a copy of the seed with nmax-155's rate set to 450.
    {
        "name": "pricing_honesty.CHEAPEST_IN_STOCK_DAY_RATE_THB ~ seed nmax-155 base rate",
        "spec": "specs/turbobaby/pricing_honesty.t27",
        "const": "CHEAPEST_IN_STOCK_DAY_RATE_THB",
        "source": "data/fleet_seed.json",
        # The family row spans three lines in the seed (key, class/body/cc, money); the rate is
        # the first field of the third. Keyed on the family, never on the value.
        "extract": ("regex", r'"key": "nmax-155",[^\n]*\n[^\n]*\n\s*"base_rate_thb_day": (\d+),'),
        "relation": "equal",
        "why": "the published pre-discount rate every divergence example in the contract is "
               "built on; a re-priced seed under a stale contract argues D11's threshold from "
               "a number the door no longer publishes",
    },
    {
        "name": "rental_terms.REF_BASE_THB_DAY ~ seed nmax-155 base rate",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "REF_BASE_THB_DAY",
        "source": "data/fleet_seed.json",
        "extract": ("regex", r'"key": "nmax-155",[^\n]*\n[^\n]*\n\s*"base_rate_thb_day": (\d+),'),
        "relation": "equal",
        "why": "the declared copy of pricing-honesty's figure, which forty-odd audit assertions "
               "compute with; bound to the same seed row as its owner so that a one-sided edit "
               "of either is red",
    },
    # Added 2026-09-24. order_money.t27 declared the seed's size as 19434 bytes, which was one
    # host's CRLF working copy: the committed blob is 19084 bytes with 350 lines, and an LF checkout
    # (CI) reads 19084. The contract now declares the blob size and the line count; this row binds
    # the line count, the one figure Python reads identically on either checkout (read_text uses
    # universal newlines). The byte size has no extractor here and stays the contract's assertion.
    # Planted RED through --source-override on a copy of the seed with one blank line appended.
    {
        "name": "order_money.FLEET_SEED_LINES ~ data/fleet_seed.json line count",
        "spec": "specs/turbobaby/order_money.t27",
        "const": "FLEET_SEED_LINES",
        "source": "data/fleet_seed.json",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the zero-mention measurements beside it were taken on a seed of this length; a "
               "seed that grows or shrinks means the zeros were taken on a different file, and "
               "the length is what makes the blob-versus-working-copy arithmetic checkable",
    },
    # --- "wc -l over the tracked sources": measurements contracts state in so many words ---------
    # Five contracts open by recording how long the files they describe are, each with
    # the sentence that it is a measurement, named here by declaration and not by line
    # (three of the four line citations that stood here had drifted off their sentences by
    # 2026-09-24): api_surface.t27 ROUTER_SOURCE_LINES, client_errors.t27 INTAKE_SOURCE_LINES,
    # events_booking.t27 HTTP_SOURCE_LINES, customer_surface.t27 WITNESS_LINES and
    # catalog_write.t27 ADMIN_SCREEN_LINES. Every census that follows in those files was
    # taken against a file of that length, so a changed length means the census was taken
    # against a different file.
    # The sentence over catalog_write.t27's ADMIN_SCREEN_LINES makes TWO claims (the length,
    # and "The screen is the largest file"), and binding one of them to a RANK was
    # a hole: `("tree_line_nth", 1)` answers "how long is the longest file under src/",
    # which is not what ADMIN_SCREEN_LINES says. Measured 2026-09-21 in an out-of-repo
    # mirror: with admin_screen.rs cut to 3000 lines, main.rs padded to 5802 and
    # orders.rs padded to 4599, the gate printed "OK - 65 bindings hold" while
    # ADMIN_SCREEN_LINES = 5802 had become a false statement about the file
    # ADMIN_SCREEN names. The two claims are now bound separately: the length against
    # the file the contract NAMES, and the ranking against the contract's own path
    # constant.
    {
        "name": "catalog_write.ADMIN_SCREEN_LINES ~ admin_screen.rs length",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "ADMIN_SCREEN_LINES",
        # The path is not spelled here by choice: catalog_write.t27 declares it as
        # ADMIN_SCREEN, and the binding below pins that declaration to the tree, so the
        # string this row reads is the one the contract is judged on.
        "source": "src/ui/screens/admin_screen.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "catalog_write.t27 argues from the size of this file (ADMIN_SCREEN_LINES, "
               "then the citers counted under it, CONTRACTS_CITING_ADMIN_SCREEN) that no "
               "contract owns its write bounds; the argument is only as current as the "
               "measurement it opens with",
    },
    {
        "name": "catalog_write.ADMIN_SCREEN ~ largest file under src/",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "ADMIN_SCREEN",
        "source": "src/**/*.rs",
        # The other half of the sentence over ADMIN_SCREEN_LINES, "The screen is the largest file".
        # tree_path_nth answers with a repo-relative PATH, so the comparison is against
        # the contract's own declared path rather than against a number that any file
        # could supply.
        "extract": ("tree_path_nth", 1),
        "relation": "equal",
        "why": "'the screen is the largest file' is a claim about WHICH file, and the "
               "length binding above cannot see it: a rank check alone stays green when "
               "another file inherits the rank, and a length check alone stays green "
               "when the screen stops being the biggest",
    },
    {
        "name": "catalog_write.NEXT_LARGEST_SOURCE_LINES ~ 2nd largest under src/",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "NEXT_LARGEST_SOURCE_LINES",
        "source": "src/**/*.rs",
        # No `expect_path` companion here on purpose: the contract records the runner-up
        # as a NUMBER and names no file for it, so pinning one would invent a fact. The
        # failure message names the file it measured instead, because "contract=4599
        # source=5000" over a glob is unreadable without it.
        "extract": ("tree_line_nth", 2),
        "relation": "equal",
        "why": "the runner-up is half the argument -- the gap between the two is what "
               "makes 'nobody owns this file's bounds' worth saying",
    },
    {
        "name": "api_surface.ROUTER_SOURCE_LINES ~ src/api/mod.rs length",
        "spec": "specs/turbobaby/api_surface.t27",
        "const": "ROUTER_SOURCE_LINES",
        "source": "src/api/mod.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the served-route census and the wiring gate at :304-494 were both taken "
               "over this file at this length",
    },
    {
        "name": "api_surface.DOCUMENT_SOURCE_LINES ~ src/api/openapi.rs length",
        "spec": "specs/turbobaby/api_surface.t27",
        "const": "DOCUMENT_SOURCE_LINES",
        "source": "src/api/openapi.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the published-document side of the same comparison; the contract is "
               "about routes served versus routes documented, so both lengths matter",
    },
    {
        "name": "client_errors.INTAKE_SOURCE_LINES ~ src/api/client_errors.rs length",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "INTAKE_SOURCE_LINES",
        "source": "src/api/client_errors.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "client_errors.t27:94 records it so that 'the whole file' is a "
               "measurement; every claim about the intake path scopes to this length",
    },
    {
        "name": "client_errors.FILTER_SOURCE_LINES ~ src/trios/js_errors.rs length",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "FILTER_SOURCE_LINES",
        "source": "src/trios/js_errors.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "same sentence, the filter half",
    },
    {
        "name": "client_errors.OVERLAY_SOURCE_LINES ~ error_overlay.rs length",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "OVERLAY_SOURCE_LINES",
        "source": "src/ui/components/error_overlay.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "same sentence, the overlay half",
    },
    {
        "name": "events_booking.HTTP_SOURCE_LINES ~ src/api/events.rs length",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "HTTP_SOURCE_LINES",
        "source": "src/api/events.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "events_booking.t27 HTTP_SOURCE_LINES records it so 'the whole file' is a measurement "
               "and not a mood, in its own words",
    },
    {
        "name": "events_booking.CALENDAR_SOURCE_LINES ~ src/trios/calendar.rs length",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "CALENDAR_SOURCE_LINES",
        "source": "src/trios/calendar.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "same sentence, the calendar trio",
    },
    {
        "name": "events_booking.ATTENDEE_SOURCE_LINES ~ src/trios/attendees.rs length",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "ATTENDEE_SOURCE_LINES",
        "source": "src/trios/attendees.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "same sentence, the attendee trio",
    },
    {
        "name": "events_booking.SAME_DAY_WITNESS_LINES ~ same-day test length",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "SAME_DAY_WITNESS_LINES",
        "source": "tests/integration_events_same_day.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the contract leans on this test as its witness; a witness that has "
               "changed length is a witness nobody has re-read",
    },
    {
        "name": "customer_surface.WITNESS_LINES ~ wiring test length",
        "spec": "specs/turbobaby/customer_surface.t27",
        "const": "WITNESS_LINES",
        "source": "tests/customer_surface_wiring.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "src/lib.rs:17-18 gates `pub mod ui` on wasm32, so cargo test compiles "
               "no line of the customer surface and this source-text scan is the ONLY "
               "instrument; the contract records its length to make that claim checkable",
    },
    {
        "name": "customer_surface.WITNESS_TESTS ~ attribute positions",
        "spec": "specs/turbobaby/customer_surface.t27",
        "const": "WITNESS_TESTS",
        "source": "tests/customer_surface_wiring.rs",
        # Column-zero anchoring is the cheap stand-in for "attribute POSITION" that the
        # paragraph over customer_surface.t27's WITNESS_TESTS_BY_NAIVE_GREP spells out: the
        # four extra hits the naive grep finds sit inside string literals and a comment,
        # all indented.
        "extract": ("regex_count", r"^#\[test\]"),
        "relation": "equal",
        "why": "how many checks the only instrument actually runs",
    },
    {
        "name": "customer_surface.WITNESS_TESTS_BY_NAIVE_GREP ~ every #[test] token",
        "spec": "specs/turbobaby/customer_surface.t27",
        "const": "WITNESS_TESTS_BY_NAIVE_GREP",
        "source": "tests/customer_surface_wiring.rs",
        "extract": ("regex_count", r"#\[test\]"),
        "relation": "equal",
        "why": "the contract records the WRONG number too, on purpose, so the next "
               "reader running the fast grep does not 'correct' a true 15 into a false "
               "19; pinning both keeps the gap between them from moving unnoticed",
    },
    # --- order presentation fixes, 2026-09-22 ---
    # person-naming family. The attendee handle bound is events_booking's alone since
    # person_naming stopped restating it, and until now nothing tied its four numbers to
    # src/trios/attendees.rs except ATTENDEE_SOURCE_LINES, which says the file's length
    # changed and not which fact did. Measured by hand 2026-09-22: the comment at src/trios/attendees.rs:39
    # reads `5..=32`, the check at src/trios/attendees.rs:41 reads `handle.len() > 32`, and
    # the module holds 16 `#[test]` attributes, all indented inside `mod tests`, none in a
    # string or a comment.
    # HANDLE_MIN_ENFORCED is NOT bound: the enforced floor is an emptiness check with no
    # numeral in it, and a pattern for "no lower length check exists" would be a second
    # fact wearing that constant's name. What a fix to the floor would change is bound
    # under its own name instead, ATTENDEE_SOURCE_LENGTH_COMPARISONS, the last entry here.
    {
        "name": "events_booking.HANDLE_MIN_DECLARED ~ attendees.rs handle comment, low end",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "HANDLE_MIN_DECLARED",
        "source": "src/trios/attendees.rs",
        "extract": ("regex", r"//\s*Telegram handles are (\d+)\.\.=\d+ of"),
        "relation": "equal",
        "why": "the contract's DECLARED minimum is what this comment states; the gap it "
               "pins against the enforced floor is only real while the comment says it",
    },
    {
        "name": "events_booking.HANDLE_MAX_DECLARED ~ attendees.rs handle comment, high end",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "HANDLE_MAX_DECLARED",
        "source": "src/trios/attendees.rs",
        "extract": ("regex", r"//\s*Telegram handles are \d+\.\.=(\d+) of"),
        "relation": "equal",
        "why": "same comment, the upper end the contract says the check agrees with",
    },
    {
        "name": "events_booking.HANDLE_MAX_ENFORCED ~ attendees.rs length check",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "HANDLE_MAX_ENFORCED",
        "source": "src/trios/attendees.rs",
        "extract": ("regex", r"\bhandle\.len\(\)\s*>\s*(\d+)"),
        "relation": "equal",
        "why": "the normaliser's upper length comparison, which the contract says the "
               "check enforces; that it is the ONLY length comparison is not this binding's "
               "to say, and ATTENDEE_SOURCE_LENGTH_COMPARISONS below says it",
    },
    {
        "name": "events_booking.ATTENDEE_SOURCE_TESTS ~ attendees.rs #[test] attributes",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "ATTENDEE_SOURCE_TESTS",
        "source": "src/trios/attendees.rs",
        "extract": ("regex_count", r"^[ \t]*#\[test\][ \t]*$"),
        "relation": "equal",
        "why": "the contract's HANDLE_GAP_EXAMPLES_IN_TESTS and HANDLE_OVER_MAX_EXAMPLES_"
               "IN_TESTS were read off these tests; a test added or removed means they "
               "must be read again, which the line count alone does not say",
    },
    # Added after a checker's plant: `if handle.len() < 5 {` written over the emptiness
    # check at src/trios/attendees.rs:36 fixes the floor, keeps 357 lines and 16 tests, and
    # left the four bindings above and ATTENDEE_SOURCE_LINES all holding. The count is of
    # ORDERING comparisons of a length anywhere in the module: `<`, `<=`, `>` or `>=` on
    # either side of a `len()` or `count()`, and a range `contains` over one. Direction is
    # not read, because `len() > N` is a ceiling in a refusal and a floor in an acceptance;
    # a first draft that counted only "from below" forms missed `.filter(|h| h.len() > 4)`.
    # Measured 2026-09-22: 1 in the file (the upper check at src/trios/attendees.rs:41), 2
    # in each of seven planted floors (the checker's, reversed, chars().count(), range
    # contains, a second comparison on the upper check's line, `>= N` and `> N` filters in
    # attendee_link), 1 in `len().lt(&N)` and `get(N..).is_none()`, which stay unseen. The
    # `(?<![=\-])` keeps `=>` and `->` from reading as comparisons. No witness: the expected
    # value is 1, so a scan gone blind reads 0 and is red on its own (D16 asks a witness of
    # a zero). Rewriting the upper check away is HANDLE_MAX_ENFORCED's to catch, not this.
    {
        "name": "events_booking.ATTENDEE_SOURCE_LENGTH_COMPARISONS ~ attendees.rs length comparisons",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "ATTENDEE_SOURCE_LENGTH_COMPARISONS",
        "source": "src/trios/attendees.rs",
        "extract": (
            "regex_count",
            r"\.(?:len|count)\(\)\s*[<>]"
            r"|(?<![=\-])[<>]=?\s*[\w.()]*\.(?:len|count)\(\)"
            r"|\.contains\(\s*&[\w.()]*\.(?:len|count)\(\)",
        ),
        "relation": "equal",
        "why": "HANDLE_MIN_ENFORCED records an emptiness check with no numeral and is bound "
               "by nothing; a floor fixed in place as a length comparison flips it while the "
               "line and test counts hold, and this count is what turns red instead",
    },
    # status family. src/trios/order_status_view.rs is the one exact reading of the order
    # status the orders list, the order detail screen and the stepper now share. The
    # vocabulary and the terminal set are order-status's, and the reading keeps named copies
    # of both, one arm per line in the shape `"name" => StatusArm::X,` or
    # `"a" | "b" => StatusArm::X,`. Measured by hand 2026-09-22: arm_of's seven named arms
    # hold the nine quoted names at src/trios/order_status_view.rs:119-125 (its eighth arm
    # is the fallback), and no other line of the file, doc comments and tests included, has
    # a quoted name followed by `=> StatusArm::`. The lookahead lets both names of a pair
    # match.
    {
        "name": "order_status.STATUS_NAMES ~ order_status_view.rs arm_of arms",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "STATUS_NAMES",
        "source": "src/trios/order_status_view.rs",
        "extract": ("regex_all", r'"([a-z_]+)"(?=(?: \| "[a-z_]+")* => StatusArm::)'),
        "relation": "set_equal",
        "why": "the customer's reading must give every accepted name a deliberate arm; a "
               "tenth name the server starts accepting would otherwise fall to Unresolved "
               "and be drawn as unreadable, and STATUS_NAMES is already bound to "
               "VALID_STATUSES, so this chains the screens to the write path",
    },
    {
        "name": "order_status.TERMINAL_NAMES ~ order_status_view.rs terminal arms",
        "spec": "specs/turbobaby/order_status.t27",
        "const": "TERMINAL_NAMES",
        "source": "src/trios/order_status_view.rs",
        "extract": (
            "regex_all",
            r'"([a-z_]+)"(?=(?: \| "[a-z_]+")* => StatusArm::'
            r"(?:DeliveredOrCompleted|CancelledOrRejected),)",
        ),
        "relation": "set_equal",
        "why": "terminality is order-status's; the reading's is_terminal is true for exactly "
               "these two arms, so the names in them are its copy of the terminal set, and a "
               "name moved in or out of them flips a chip, the reorder button and the bar "
               "(the host tests in the module check is_terminal against the same constant)",
    },
    # LABEL_KEY_NAMES is order-presentation's own table, in the order of its ARM_* arms.
    # label_key's eight arms are the only `StatusArm::X => T_ORDERS_STATUS_...,` lines in the
    # file (src/trios/order_status_view.rs:146-153), written in that order.
    {
        "name": "order_presentation.LABEL_KEY_NAMES ~ order_status_view.rs label_key arms",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "LABEL_KEY_NAMES",
        "source": "src/trios/order_status_view.rs",
        "extract": ("regex_all", r"StatusArm::\w+ => (T_ORDERS_STATUS_[A-Z_]+),"),
        "relation": "list_equal",
        "why": "a label key the owner adds when splitting a shared arm (owner questions A "
               "and B) has to arrive in the contract's table too, in arm order; until then "
               "the table and the one function that uses it cannot drift apart",
    },
    # The three UI files hold no status literal at all since the repair: every decision is
    # the reading's. Counted over RAW text, comments included, so prose quoting a name is a
    # finding too. Measured 2026-09-22: 0, 0 and 0 (28, 23 and 33 at 14b01ac). The witness
    # is the call of the one reading, which each file must make (D16: a zero needs a proof
    # the scan can still see). The nine names in the pattern are order-status's STATUS_NAMES,
    # bound to the server's VALID_STATUSES above; a tenth trips the arm_of binding first.
    {
        "name": "order_presentation.OWNED_SCREEN_STATUS_LITERAL_COUNT ~ orders_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "OWNED_SCREEN_STATUS_LITERAL_COUNT",
        "source": "src/ui/screens/orders_screen.rs",
        "extract": (
            "regex_count",
            r'"(?:pending|confirmed|preparing|ready|out_for_delivery|delivered|completed|'
            r'rejected|cancelled)"',
        ),
        "witness": r"arm_of\(",
        "relation": "equal",
        "why": "a quoted status name back in the list is a second reading of the string, the "
               "defect the repair removed (a chip decided by one rule and a label by another)",
    },
    {
        "name": "order_presentation.OWNED_SCREEN_STATUS_LITERAL_COUNT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "OWNED_SCREEN_STATUS_LITERAL_COUNT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": (
            "regex_count",
            r'"(?:pending|confirmed|preparing|ready|out_for_delivery|delivered|completed|'
            r'rejected|cancelled)"',
        ),
        "witness": r"arm_of\(",
        "relation": "equal",
        "why": "the detail screen's cancel button compared the raw string with one name while "
               "its label folded case; a literal back here is that split coming back",
    },
    {
        "name": "order_presentation.OWNED_SCREEN_STATUS_LITERAL_COUNT ~ status_stepper.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "OWNED_SCREEN_STATUS_LITERAL_COUNT",
        "source": "src/ui/components/status_stepper.rs",
        "extract": (
            "regex_count",
            r'"(?:pending|confirmed|preparing|ready|out_for_delivery|delivered|completed|'
            r'rejected|cancelled)"',
        ),
        "witness": r"arm_of\(",
        "relation": "equal",
        "why": "the stepper drew an unreadable status as a finished delivery and printed the "
               "server's word as a step label; it may only draw what the reading decides, and "
               "its pipeline now lives in the host-compiled module",
    },
    # wc -l of the four files the contract owns, measured 2026-09-22 after cargo fmt. The
    # money and cancellation families edit the same two screens next, and every site the
    # contract cites was measured against files of these lengths.
    {
        "name": "order_presentation.LIST_LINE_COUNT ~ orders_screen.rs length",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "LIST_LINE_COUNT",
        "source": "src/ui/screens/orders_screen.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the list's sites in the contract were measured against a file of this length",
    },
    {
        "name": "order_presentation.DETAIL_LINE_COUNT ~ order_detail_screen.rs length",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "DETAIL_LINE_COUNT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the detail screen's sites in the contract were measured against a file of "
               "this length",
    },
    {
        "name": "order_presentation.SHARED_COMPONENT_LINE_COUNT ~ status_stepper.rs length",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "SHARED_COMPONENT_LINE_COUNT",
        "source": "src/ui/components/status_stepper.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the stepper went from 122 lines to 72 when it stopped deciding; growth is the "
               "first sign it has started deciding again",
    },
    {
        "name": "order_presentation.READING_LINE_COUNT ~ order_status_view.rs length",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "READING_LINE_COUNT",
        "source": "src/trios/order_status_view.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "the reading's sites in the contract, and in order-status's DISPLAY_PIPELINE_"
               "MODEL_A, were measured against a file of this length",
    },
    # The owner's answer 3 of 2026-09-25: a line of the old catalogue is served under the
    # neutral name with its stored figures only. The rule is src/trios/legacy_view.rs; the
    # contract names the name field it writes, the figures it keeps and how many bonus types
    # keep their description. Measured by hand 2026-09-25: strain_name, [quantity,
    # unit_price] and 4. (The key the name is looked up by is an identifier, which this reader
    # does not evaluate; tests/legacy_view_wiring.rs holds it to the contract.)
    {
        "name": "order_presentation.RETIRED_LINE_NAME_FIELD ~ legacy_view.rs MASKED_LINE_NAME_KEY",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "RETIRED_LINE_NAME_FIELD",
        "source": "src/trios/legacy_view.rs",
        "extract": ("regex", r'pub const MASKED_LINE_NAME_KEY: &str = ("[a-z_]+");'),
        "relation": "equal",
        "why": "the field is the one every owned screen reads first; another field would leave "
               "a bundle that predates the rule printing its own fallback",
    },
    {
        "name": "order_presentation.RETIRED_LINE_KEPT_FIELDS ~ legacy_view.rs KEPT_LINE_KEYS",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "RETIRED_LINE_KEPT_FIELDS",
        "source": "src/trios/legacy_view.rs",
        "extract": ("regex_list", r"pub const KEPT_LINE_KEYS: \[&str; \d+\] = \[([^\]]*)\];"),
        "relation": "list_equal",
        "why": "only the quantity and the unit price may pass as stored; a key added here would "
               "serve a stored id or name of the old catalogue to its customer again",
    },
    # The name of a bike line (2026-09-26): the contract names the two wire fields the rule
    # reads, in the order it tries them, and the rule reads them from its own list. Measured by
    # hand 2026-09-26: [bike_name, bike_key].
    {
        "name": "order_presentation.BIKE_LINE_NAME_FIELDS ~ order_line.rs BIKE_LINE_NAME_FIELDS",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "BIKE_LINE_NAME_FIELDS",
        "source": "src/trios/order_line.rs",
        "extract": (
            "regex_list",
            r"pub const BIKE_LINE_NAME_FIELDS: \[&str; \d+\] = \[([^\]]*)\];",
        ),
        "relation": "list_equal",
        "why": "the stored name before the family key is BikeLine's own documented fallback; "
               "another order, or a third field, would name a bike line by something the line "
               "does not say",
    },
    {
        "name": "legacy_retirement.OWNER_ANSWER_3_DESCRIBED_TX_TYPE_COUNT ~ legacy_view.rs",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "OWNER_ANSWER_3_DESCRIBED_TX_TYPE_COUNT",
        "source": "src/trios/legacy_view.rs",
        "extract": ("regex", r"pub const DESCRIBED_TX_TYPES: \[&str; (\d+)\]"),
        "relation": "equal",
        "why": "a bonus type added to the served list serves its stored descriptions, which "
               "nothing here can tell from the previous shop's",
    },
    # Measured by hand 2026-09-25 (UTC): 2 (garden_harvest, garden_reward).
    {
        "name": "legacy_retirement.OWNER_ANSWER_3_WITHHELD_TX_TYPE_COUNT ~ legacy_view.rs",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "OWNER_ANSWER_3_WITHHELD_TX_TYPE_COUNT",
        "source": "src/trios/legacy_view.rs",
        "extract": ("regex", r"pub const GARDEN_ERA_TX_TYPES: \[&str; (\d+)\]"),
        "relation": "equal",
        "why": "a garden-era type dropped from the list is served as stored, and every bundle "
               "cached before answer 3 labels it with the garden's label again",
    },
    # cancel family. The order detail screen sent POST /api/orders/:id/cancel, bound the
    # answer to `let _resp` and closed its dialog right after the await, on every path.
    # Measured by hand 2026-09-22 with these exact patterns: at 14b01ac (git show) 1 and 1,
    # on the repaired screen 0 and 0. The first counts a POST whose result is bound to an
    # underscore name -- `[^;]*` spans the builder chain across lines. The second counts a
    # statement setting a dialog-visibility signal (`*confirm*`) to false right after
    # ANOTHER statement or block; the two closes left are each the first statement of their
    # block (the verdict's `if` and the customer's own "no"), so neither counts. Both are
    # zeros, so each carries a witness the repaired screen must hold (D16).
    {
        "name": "order_presentation.CANCEL_POSTS_WITH_A_DISCARDED_ANSWER ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "CANCEL_POSTS_WITH_A_DISCARDED_ANSWER",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("regex_count", r"let\s+_\w*\s*=[^;]*\.post\("),
        "witness": r"order_cancel_answer\(",
        "relation": "equal",
        "why": "a customer's cancellation whose answer nobody reads tells a refusal, a lost "
               "connection and a success alike, which is the defect the repair removed",
    },
    {
        "name": "order_presentation.DIALOG_CLOSES_THAT_FOLLOW_ANOTHER_STATEMENT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "DIALOG_CLOSES_THAT_FOLLOW_ANOTHER_STATEMENT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("regex_count", r"[;}]\s*\w*confirm\w*\.set\(false\)"),
        "witness": r"if order_cancel_dialog_closes\(",
        "relation": "equal",
        "why": "AGENTS.md lesson 4: the dialog closes on the server's confirmation; a close "
               "written after the send's await closes it on every answer again",
    },
    # The sentences are client-errors'. order_cancel_line in src/trios/api_errors.rs picks
    # its own keys in arms of the shape `t(lang, T_KEY).to_string(), OrderCancelTone::X,`
    # (rustfmt puts the tone on the next line; `\s*` spans it), and no other line of the
    # file, tests included, has that shape: measured 2026-09-22, six captures. Re-measured
    # after the server class was held for a fresh reading (the unknown-outcome arms now come
    # first), same six in file order: T_SUCCESS_STATUS_LOADING, T_SUCCESS_STATUS_ERROR,
    # T_ORDER_DETAIL_CANCELLED_BY_USER, T_ORDER_DETAIL_NOT_FOUND, T_API_ERR_UNKNOWN,
    # T_CHECKOUT_ERR_NETWORK. Re-measured 2026-09-25, when question D's key arrived for
    # the 409 arm: seven, T_ORDER_DETAIL_CANCEL_REFUSED between T_ORDER_DETAIL_NOT_FOUND
    # and T_API_ERR_UNKNOWN (the new test in that file names the key in a `let`, not in
    # this shape, so it adds no capture).
    {
        "name": "client_errors.ORDER_CANCEL_OWN_KEY_NAMES ~ api_errors.rs order_cancel_line arms",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "ORDER_CANCEL_OWN_KEY_NAMES",
        "source": "src/trios/api_errors.rs",
        "extract": (
            "regex_all",
            r"t\(lang, (T_[A-Z0-9_]+)\)\.to_string\(\),\s*OrderCancelTone::",
        ),
        "relation": "set_equal",
        "why": "every sentence this surface speaks is a key the contract names, the one "
               "written for question D on 2026-09-25 included; a key added or swapped here "
               "has to be recorded where the sentence is owned",
    },
    # The delegation to the general mapper, counted as every NON-TEST call in the module
    # that defines it, whatever its spelling and whichever function holds it. Corrected
    # after a checker's plant: the first pattern counted one tuple shape only, so a second
    # delegation spelled `(friendly_response_error(lang, 409), ...)` left it at 1. The
    # lookahead admits a call only when the module's column-0 `#[cfg(test)]` still follows
    # it (src/trios/api_errors.rs has exactly one, above `mod tests`, which is the file's
    # last item); `(?<!fn )` drops the definition. Measured by hand 2026-09-22: 1, the arm
    # of order_cancel_line for the identity gates, the limiter and the server class, with
    # the tests' eleven calls all below the marker (0 at a33e500, before the cancel surface
    # existed). The module count after it closes the rest of src/trios: exactly one trio
    # file of 23 calls the mapper at all, tests included, and it is this one (i18n.rs
    # names it in a comment, without a call). The checker's plant delegate_409 reads 2.
    {
        "name": "client_errors.API_MAPPER_DELEGATION_SITES ~ api_errors.rs non-test calls",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "API_MAPPER_DELEGATION_SITES",
        "source": "src/trios/api_errors.rs",
        "extract": (
            "regex_count",
            r"(?<!fn )\bfriendly_response_error\((?=[\s\S]*^#\[cfg\(test\)\])",
        ),
        "relation": "equal",
        "why": "the ten call sites the contract counts are in src/ui/screens; this is the one "
               "place a trio hands a customer surface to the general mapper, and a second one "
               "would reach its cart and price sentences from somewhere nobody counted",
    },
    {
        "name": "client_errors.TRIO_MODULES_CALLING_THE_API_MAPPER ~ src/trios calls",
        "spec": "specs/turbobaby/client_errors.t27",
        "const": "TRIO_MODULES_CALLING_THE_API_MAPPER",
        "source": "src/trios/**/*.rs",
        "extract": ("tree_module_count", r"(?<!fn )\bfriendly_response_error\("),
        "relation": "equal",
        "why": "the binding above counts one module; a delegation added to another trio "
               "would sit outside it, and this count is what turns red instead",
    },
    # The dialog's closes, each accounted for by where it stands. A checker's plant put a
    # close first thing in Confirm's onclick, `{ show_cancel_confirm.set(false); ... }`:
    # it follows `{`, so DIALOG_CLOSES_THAT_FOLLOW_ANOTHER_STATEMENT above never saw it,
    # and the line count held. Measured by hand 2026-09-22 on the repaired screen: 2
    # writes of `false` to a `*confirm*` signal in the whole file (:343 and :577), one
    # of them the whole body of the verdict's `if` and the other the whole body of the
    # "no" button's onclick with that button's label on the next line; at 14b01ac, 2, 0
    # and 1 (the old close followed the await, and "no" was already written this way).
    # A close added anywhere moves the first count; a close moved off the verdict or off
    # "no" moves one of the other two.
    {
        "name": "order_presentation.CANCEL_DIALOG_CLOSES ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "CANCEL_DIALOG_CLOSES",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("regex_count", r"\w*confirm\w*\.set\(false\)"),
        "relation": "equal",
        "why": "AGENTS.md lesson 4: the dialog closes on the server's confirmation or on the "
               "customer's own no; a third close is a close before the server has answered",
    },
    {
        "name": "order_presentation.CANCEL_DIALOG_CLOSES_UNDER_THE_VERDICT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "CANCEL_DIALOG_CLOSES_UNDER_THE_VERDICT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": (
            "regex_count",
            r"if order_cancel_dialog_closes\(\w+\) \{\s*\w*confirm\w*\.set\(false\);\s*\}",
        ),
        "relation": "equal",
        "why": "the one close that answers the server must be the verdict's whole body, so "
               "that a success and nothing else closes the dialog",
    },
    {
        "name": "order_presentation.CANCEL_DIALOG_CLOSES_ON_THE_CUSTOMERS_NO ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "CANCEL_DIALOG_CLOSES_ON_THE_CUSTOMERS_NO",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": (
            "regex_count",
            r'onclick: move \|_\| \{ \w*confirm\w*\.set\(false\); \},\s*"\{t\(lang, T_MODAL_CANCEL\)\}"',
        ),
        "relation": "equal",
        "why": "the other close is the customer's own no, whose onclick does nothing else; "
               "moved to Confirm, it would close the dialog before anything is sent",
    },
    # money family. Defects 2 and 6: four of the five money figures on the two order
    # screens went straight into format_baht, and a figure declared as a bare number lost
    # the screen when a payload omitted it. The rule now lives in src/trios/pricing.rs
    # (order_money_text / order_total_text over measured_money), host-tested there; these
    # six bind the screens' side of it. Measured by hand 2026-09-22 with these exact
    # patterns: on the screens before the money repair (the working tree after the status
    # and cancel repairs, rebuilt as a plant) 3, 1, 4, 1, 2 -- and 0 on the checkout, which
    # has never built a bike line -- and 0 on every one of the six since. All six are zeros,
    # so each carries a witness the repaired file must hold (D16). The format_baht count
    # reads RAW text, comments included, like every regex_count here: prose naming the
    # formatter on a screen is a finding too, and neither screen holds any.
    {
        "name": "order_presentation.DETAIL_FORMAT_BAHT_OCCURRENCE_COUNT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "DETAIL_FORMAT_BAHT_OCCURRENCE_COUNT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("regex_count", r"format_baht\("),
        "witness": r"crate::trios::pricing::order_money_text\(",
        "relation": "equal",
        "why": "format_baht takes a bare f64 and cannot say absent; an order figure handed to "
               "it here bypasses the order rule, which is defect 2 coming back",
    },
    {
        "name": "order_presentation.LIST_FORMAT_BAHT_OCCURRENCE_COUNT ~ orders_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "LIST_FORMAT_BAHT_OCCURRENCE_COUNT",
        "source": "src/ui/screens/orders_screen.rs",
        "extract": ("regex_count", r"format_baht\("),
        "witness": r"crate::trios::pricing::order_total_text\(",
        "relation": "equal",
        "why": "the list prints the same stored total as the detail; formatting it here a "
               "second way is the two-readings drift the order rule exists to stop",
    },
    # A figure field declared as a bare number, at the start of a line: `subtotal`,
    # `bonus_used` or `total` as f64, `stars_used` as i64. `quantity: f64` on the line DTO is
    # not money and does not match. The witness is the flattened Option block.
    {
        "name": "order_presentation.DETAIL_BARE_FIGURE_FIELD_COUNT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "DETAIL_BARE_FIGURE_FIELD_COUNT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": (
            "regex_count",
            r"^[ \t]*(?:subtotal|bonus_used|total):[ \t]*f64,|^[ \t]*stars_used:[ \t]*i64,",
        ),
        "witness": r"#\[serde\(flatten\)\]\s*money: crate::trios::pricing::OrderMoney,",
        "relation": "equal",
        "why": "a bare figure fails the whole response when a payload omits it, and the "
               "customer is told the order was not found (defect 6, the lost screen)",
    },
    {
        "name": "order_presentation.LIST_BARE_FIGURE_FIELD_COUNT ~ orders_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "LIST_BARE_FIGURE_FIELD_COUNT",
        "source": "src/ui/screens/orders_screen.rs",
        "extract": ("regex_count", r"^[ \t]*total:[ \t]*f64,"),
        "witness": r"#\[serde\(default\)\]\s*total: Option<f64>,",
        "relation": "equal",
        "why": "on the list one order without a total failed the WHOLE response, so every "
               "order vanished behind the error text",
    },
    # A discount row decided by the screen's own comparison of a figure with zero -- the
    # shape `if order.bonus_used > 0.0` had, which hid a negative bonus exactly like a zero
    # one. `[^\n;{]*` keeps the match on one expression. The witness is the rule's own row.
    {
        "name": "order_presentation.DETAIL_ROW_COMPARISON_COUNT ~ order_detail_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "DETAIL_ROW_COMPARISON_COUNT",
        "source": "src/ui/screens/order_detail_screen.rs",
        "extract": ("regex_count", r"(?:bonus_used|stars_used)\b[^\n;{]*>\s*0"),
        "witness": r"if let Some\(bonus\) = &money\.bonus \{",
        "relation": "equal",
        "why": "a measured zero omits its row and an absence keeps it with the dash; a "
               "comparison written on the screen folds the two together again",
    },
    # The reachability premise of owner question F: no shipped client submits a bike line,
    # so an order holding one is possible on the wire and absent from traffic. The checkout
    # builds its lines in one map (src/ui/screens/checkout_screen.rs, items_json), four
    # shapes, none with a `bike` key; `git log -S'"bike":' -- src/ui/` finds none ever
    # added. A bike key appearing there makes the question current rather than theoretical.
    {
        "name": "order_presentation.CHECKOUT_BIKE_LINE_KEY_COUNT ~ checkout_screen.rs",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "CHECKOUT_BIKE_LINE_KEY_COUNT",
        "source": "src/ui/screens/checkout_screen.rs",
        "extract": ("regex_count", r'"bike"\s*:'),
        "witness": r'"set_id": item\.id',
        "relation": "equal",
        "why": "the owner question on a bike line's order total is recorded as reachability, "
               "not traffic; a client submitting bike lines turns it into live traffic",
    },
    # --- commerce group: checkout contact, commerce, deposit tiers, cart persistence, delivery terms ---
    # Added 2026-09-22. Each row was measured both sides by hand at a33e500 and went RED on a
    # planted copy (--source-override) for at least one drift its `why` names. Not for every
    # drift a `why` names: review the same day planted the likeliest remaining one per row and
    # found two green -- DEAL_NAMES on a new variant no test asserts, API_KINDS on a default
    # arm turned permissive. Each now has a companion row or a witness that was planted RED on
    # exactly that drift; what a row still cannot see is written in its comment, not claimed.
    {
        "name": "checkout_contact.PHONE_DIGITS_MAX ~ store.rs normalizer digit ceiling",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "PHONE_DIGITS_MAX",
        "source": "src/trios/store.rs",
        "extract": ("regex", r"if digits\.len\(\) < \d+ \|\| digits\.len\(\) > (\d+)\s*\{"),
        "relation": "equal",
        "why": "normalize_phone is the one shape test the button AND the server run "
               "(src/api/orders.rs:131 calls it), so this ceiling decides whose number can "
               "place an order at all; checkout_contact.t27:139-143 pins it as E.164's",
    },
    {
        "name": "checkout_contact.PHONE_DIGITS_MIN ~ store.rs normalizer digit floor",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "PHONE_DIGITS_MIN",
        "source": "src/trios/store.rs",
        "extract": ("regex", r"if digits\.len\(\) < (\d+) \|\| digits\.len\(\) > \d+\s*\{"),
        "relation": "equal",
        "why": "the floor of the same guard, which checkout_contact.t27:140-146 records as "
               "this repository's own rule and not E.164's -- nothing else says why it is "
               "five, and a raised floor is a bare 400 at the server for a real number",
    },
    {
        "name": "checkout_contact.NAME_CHARS_MAX ~ store.rs button and validator caps",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "NAME_CHARS_MAX",
        "source": "src/trios/store.rs",
        # Two sites, each pinned to the STRING it measures: the button (:467) caps the RAW
        # name beside a trimmed emptiness test, the validator (:505) caps the TRIMMED one.
        # Either site switching strings drops the count to 1: RED. The trim asymmetry is
        # checkout_contact.t27 section (4)'s subject, and its bools cannot be bound.
        "extract": ("regex", r"(?:name\.trim\(\)\.is_empty\(\) \|\| name\.len\(\)|if name\.trim\(\)\.len\(\)) > (\d+)"),
        "occurrences": 2,
        "relation": "equal",
        "why": "checkout_contact.t27:301-311 says the button and the validator carry the SAME "
               "cap over DIFFERENT strings -- raw at the button, trimmed at the handler -- and "
               "builds its padding split on it; a second number, or a button that starts to "
               "trim, is a case the contract does not describe",
    },
    {
        "name": "checkout_contact.ADDRESS_CHARS_MAX ~ store.rs button and validator caps",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "ADDRESS_CHARS_MAX",
        "source": "src/trios/store.rs",
        # :473 (raw, and AFTER the `)` that closes the mode test) and :522 (trimmed). The
        # `is_empty())` spelling is what pins the cap outside the mode test: moved inside
        # `requires_address() && (...)`, the button copy stops matching: RED.
        "extract": ("regex", r"(?:address\.trim\(\)\.is_empty\(\)\) \|\| address\.len\(\)|if address\.trim\(\)\.len\(\)) > (\d+)"),
        "occurrences": 2,
        "relation": "equal",
        "why": "same pair for the address; the_address_cap_sits_outside_the_mode_test and "
               "the padding split are both written against this one number, measured raw at "
               "the button outside the mode test and trimmed at the handler",
    },
    # Added 2026-09-25 with the owner's removal of the 20+ box (second list, answer 1): the two
    # places the removal is a number. The pushes are counted only in checkout_blockers' own
    # `out.push(` shape; the tests build their expected lists with `vec![`, so they are not.
    {
        "name": "checkout_contact.BLOCKER_COUNT ~ store.rs checkout_blockers pushes",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "BLOCKER_COUNT",
        "source": "src/trios/store.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"\bout\.push\(CheckoutBlocker::\w+\)"),
        "relation": "equal",
        "why": "the button's closed set of reasons; the age box left it on 2026-09-25 (the "
               "owner, for now), and a sixth push is a reason the contract does not name",
    },
    {
        "name": "checkout_contact.AGE_FIELD_COMPARISONS_IN_THE_ORDERS_MODULE ~ orders.rs absence",
        "spec": "specs/turbobaby/checkout_contact.t27",
        "const": "AGE_FIELD_COMPARISONS_IN_THE_ORDERS_MODULE",
        "source": "src/api/orders.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"\bage_confirmed\s*[!=]="),
        # The expected value is ZERO, so the scan must be shown to see the field before the
        # zero is believed (D16): the line that stores it as sent.
        "witness": r"age_confirmed: Set\(req\.age_confirmed\.unwrap_or\(false\)\)",
        "relation": "equal",
        "why": "SERVER_REQUIRES_AGE is false since the owner's answer of 2026-09-25; the "
               "refusal it replaced was `req.age_confirmed != Some(true)`, and a comparison of "
               "that field back in the orders module is that refusal's shape returning",
    },
    # commerce.t27 declares FULFILLMENT_NAMES once and names neither the client's
    # Fulfillment::as_str (src/trios/store.rs) nor the server's admission list as a second
    # home for it, so it is bound to ONE source: the server's, which is what a bike line
    # has to pass. A store.rs row measured green and was not added (2026-09-22).
    {
        "name": "commerce.FULFILLMENT_NAMES ~ orders.rs bike-line fulfillment admission",
        "spec": "specs/turbobaby/commerce.t27",
        "const": "FULFILLMENT_NAMES",
        "source": "src/api/orders.rs",
        "extract": ("regex_list", r'item\.fulfillment\.as_deref\(\)\s*\{\s*if !matches!\(f,\s*(.*?)\)\s*\{'),
        "relation": "set_equal",
        "why": "the admission list for a NAMED handover mode on a bike line (D7, "
               "validate_bike_lines :316-320); a mode the contract names and this list drops "
               "is a 400 on every such order, and a mode added here is one "
               "commerce_fulfillment_valid has never judged. Spelling only: a bike line that "
               "names no mode skips the `if let Some(f)` and is admitted untested",
    },
    {
        "name": "commerce.DEAL_NAMES ~ db/orders.rs serde kind asserts",
        "spec": "specs/turbobaby/commerce.t27",
        "const": "DEAL_NAMES",
        "source": "src/db/orders.rs",
        # BikeDeal's tags come from serde rename_all over the variant names and are spelled
        # nowhere but these two tests (:1962, :2052), which cargo test holds to the enum.
        # This row reads the TESTS, not the enum: a variant added with no test asserting its
        # tag leaves it green (planted 2026-09-22). The row below counts the variants.
        "extract": ("regex_all", r'assert_eq!\(json(?:\["[a-z_]+"\])*\["kind"\],\s*"(bike_[a-z_]+)"\);'),
        "relation": "set_equal",
        "why": "the wire tags of the two tagged deal shapes; binding the tests' literals "
               "closes contract -> test -> serde for the SPELLINGS, so a renamed variant whose "
               "test was updated with it moves this row. A new variant is seen by the "
               "variant count below, not here",
    },
    {
        "name": "commerce.DEAL_KIND_COUNT ~ db/orders.rs BikeDeal variants",
        "spec": "specs/turbobaby/commerce.t27",
        "const": "DEAL_KIND_COUNT",
        "source": "src/db/orders.rs",
        # Added 2026-09-22. Every variant line of `pub enum BikeDeal` at four spaces -- a
        # struct or tuple variant by its opening brace or paren, a unit variant by its
        # trailing comma, which rustfmt always writes (CI runs `cargo fmt -- --check`), so
        # a LAST unit variant with no comma is not seen -- confined to that enum's body by the lookahead: the next
        # column-zero `}` must close the item that stands right before `pub enum
        # DepositForm`. Keyed on that NAME, not on the doc words between them, so rewording
        # the doc cannot blind it; an item inserted between the two enums reads 0: a noisy
        # red, but a red. Measured 2 (:579 BikeRental, :601 BikeSale). Planted RED: a third
        # struct variant and a third unit variant, neither with a test.
        "extract": ("regex_count",
                    r"^    [A-Z]\w*\s*(?:\{|\(|,)(?=(?:(?!^\}).)*?^\}\s*(?:///[^\n]*\n\s*)*"
                    r"(?:#\[[^\n]*\n\s*)*pub enum DepositForm\b)"),
        "relation": "equal",
        "why": "serde tags every BikeDeal variant on its own, so a third deal kind reaches "
               "the wire the moment it compiles, with or without a test; the contract's "
               "exactly-two-shapes invariant is written against this count, and DEAL_NAMES "
               "is held to it by an assert in the contract",
    },
    # Added 2026-09-24 with T27 C3. deposit_refusal (src/api/orders.rs) returns its three
    # refusals as `return Some(DepositRefusal::X` lines, in the order it decides them; no other
    # line of the file, tests included, returns one (the tests assert with `Some(Deposit...`
    # and never `return`). Measured by hand: NotPublished, NotComparable, Differs. Planted
    # RED: the Differs return moved above the currency test, and the NotPublished return
    # commented out in place. Blind spot, named on review 2026-09-24: NOT_IN_A_LINE_COMMENT
    # refuses only `//`, so a return inside a /* block comment */ is still read as live and
    # the row stays green over it. What the row never proves is that the verdict is CALLED:
    # tests/deposit_check_wiring.rs binds CLIENT_DEPOSIT_RECONCILIATION_IS_NOT_SHIPPED = false
    # to the rental_deposit_refusal( call inside check_bike_lines (a bool, which this table's
    # compare() refuses to bind against a count).
    {
        "name": "commerce.CLIENT_DEPOSIT_REFUSALS ~ orders.rs deposit_refusal returns",
        "spec": "specs/turbobaby/commerce.t27",
        "const": "CLIENT_DEPOSIT_REFUSALS",
        "source": "src/api/orders.rs",
        "extract": ("regex_all", NOT_IN_A_LINE_COMMENT + r"return Some\(DepositRefusal::(\w+)"),
        "relation": "list_equal",
        "why": "the order the verdict decides in is the rule: money on a family with no "
               "published deposit is refused before any figure is read, and a figure in a "
               "currency no published rate converts is refused before it can be compared "
               "with a baht figure; a refusal dropped, added or reordered here changes which "
               "deposits an order may carry without the contract saying so",
    },
    # deposit_tiers.t27 is cited by NAME below, not by line: this change also edits its
    # wiring block, which moves every line after it.
    {
        "name": "deposit_tiers.TIER_3_THB ~ 082 xmax-300-new deposit",
        "spec": "specs/turbobaby/deposit_tiers.t27",
        "const": "TIER_3_THB",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex", r"\('xmax-300-new',\s*'[^']*',\s*'[^']*',\s*(?:NULL|'[^']*'),\s*'[a-z]+',\s*'[^']*',\s*\d+,\s*(?:NULL|[0-9.]+),\s*([0-9.]+),"),
        "relation": "equal",
        "why": "deposit_tiers.t27's refutation (a) of issue #18 rests on xmax-300-new "
               "carrying this rung where xmax-300 carries TIER_2_THB at the same (class, cc), "
               "and its header prices the mistake at 2000 baht; this pins the seeded catalog "
               "deposit to the rung. The lookup's family-to-rung mapping "
               "(deposit_thb_of_family) is executed by scripts/execute_t27_assertions.py, "
               "not read here",
    },
    {
        "name": "deposit_tiers.TIER_2_THB ~ 082 xmax-300 deposit",
        "spec": "specs/turbobaby/deposit_tiers.t27",
        "const": "TIER_2_THB",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex", r"\('xmax-300',\s*'[^']*',\s*'[^']*',\s*(?:NULL|'[^']*'),\s*'[a-z]+',\s*'[^']*',\s*\d+,\s*(?:NULL|[0-9.]+),\s*([0-9.]+),"),
        "relation": "equal",
        "why": "the other half of the same pair: seeded at 7000 the two rows would collapse, "
               "a function of (class, cc) would become writable again, and the contract's "
               "2000-baht under-collection would describe a fleet that no longer exists",
    },
    {
        "name": "deposit_tiers.NOT_OFFERED_KEYS ~ 082 offered FALSE rows",
        "spec": "specs/turbobaby/deposit_tiers.t27",
        "const": "NOT_OFFERED_KEYS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_all", FAMILY_ROW_SQL + r"\s*'[^']*',\s*\d+,\s*(?:NULL|[0-9.]+),\s*(?:NULL|[0-9.]+),\s*(?:NULL|[0-9.]+),\s*FALSE,"),
        "relation": "list_equal",
        "why": "D12: the family that must not be offered. A second family seeded offered=FALSE "
               "lengthens the list, and click-125 flipped to TRUE empties it, which regex_all "
               "refuses as a measurement (D16)",
    },
    {
        "name": "deposit_tiers.FAMILIES_WITHOUT_PUBLISHED_DEPOSIT ~ 082 NULL-deposit family rows",
        "spec": "specs/turbobaby/deposit_tiers.t27",
        "const": "FAMILIES_WITHOUT_PUBLISHED_DEPOSIT",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_count", FAMILY_ROW_SQL + r"\s*'[^']*',\s*\d+,\s*(?:NULL|[0-9.]+),\s*NULL,"),
        "relation": "equal",
        "why": "D9 at the column: exactly one seeded family publishes no deposit, as NULL. A "
               "0.0 in its place renders as 'no deposit owed' and drops this count to zero; "
               "the contract's DEPOSIT_ABSENT marker exists for this one row",
    },
    {
        "name": "cart_persistence.ABANDON_FIRST_RUNG_MINUTES ~ cart_abandonment.rs first-rung call",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "ABANDON_FIRST_RUNG_MINUTES",
        "source": "src/cart_abandonment.rs",
        # One anchor from the shared predicate (:97) to the first-rung call (:172): the
        # NUMBER is the call's, its UNIT (minutes) and its CLOCK COLUMN (updated_at) are the
        # predicate's, and first_rung_due reads it as minutes_since_updated_at. The source's
        # reminder_count argument (0) is spelled too, so re-pointing the call is RED. The
        # contract's own ABANDON_FIRST_RUNG_REMINDER_COUNT is NOT read by this row.
        "extract": ("regex", r"c\.updated_at < \(now\(\) - interval '1 minute' \* \$2\)"
                             r'.*?load_abandoned_carts\(orm,\s*0,\s*(\d+),\s*""\)'),
        "relation": "equal",
        "why": "how long a cart sits before the bot messages its owner, counted in minutes "
               "against carts.updated_at -- the column cart_persistence.t27:17-29 shows is "
               "written once and never updated, so the threshold is ROW AGE, not activity; "
               "first_rung_due is written against this figure",
    },
    {
        "name": "cart_persistence.ABANDON_SECOND_RUNG_HOURS_AFTER_FIRST ~ cart_abandonment.rs nudge_after_hours",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "ABANDON_SECOND_RUNG_HOURS_AFTER_FIRST",
        "source": "src/cart_abandonment.rs",
        # The literal (:222) AND where it is spent (:223-230): reminder_count 1, zero
        # minutes, and an HOUR interval against first_reminder_sent_at. A bare
        # `nudge_after_hours = (\d+)i64` stays green when the interval becomes minutes or
        # the clock moves to reminder_sent_at; this spelling goes RED on either.
        "extract": ("regex", r"""let nudge_after_hours = ([0-9_]+)i64;\s*let carts = load_abandoned_carts\(\s*orm,\s*1,\s*0,\s*&format!\(\s*"AND c\.first_reminder_sent_at < \(now\(\) - interval '1 hour' \* \{\}\)",\s*nudge_after_hours"""),
        "relation": "equal",
        "why": "when the second message reaches the customer -- the one whose copy promises "
               "bonus points nothing credits (PROMISED_PERK_REFUSAL_NOTE); second_rung_due is "
               "written against this figure as hours since the FIRST reminder",
    },
    {
        "name": "cart_persistence.API_KINDS ~ cart.rs parse_kind arms",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "API_KINDS",
        "source": "src/api/cart.rs",
        # EVERY quoted literal on a line that goes on to `=> Ok(`, so an alias arm
        # (`"strain" | "strains" => Ok("strain")`) adds a kind instead of hiding one; the
        # literal inside Ok(...) is not followed by `=> Ok(` and is not counted. Measured 4,
        # all inside parse_kind (:102-105). A string arm elsewhere in cart.rs also reddens it.
        "extract": ("regex_all", r'"([a-z_]+)"(?=[^\n]*=> Ok\()'),
        # The arms say which kinds are admitted; only the default arm says the rest are
        # REFUSED, and the extractor above cannot see it: `_ => Ok("strain")` admits every
        # kind -- bike_rental and the deal kinds included -- and left this row green
        # (planted 2026-09-22). The witness pins the refusing default as parse_kind's last
        # arm, right before the match closes; the same plant is now RED.
        "witness": r"fn parse_kind\([^)]*\)[^{]*\{\s*match kind \{[^}]*_ => Err\(StatusCode::BAD_REQUEST\),\s*\}",
        # set_equal: API_KINDS is used by membership only (kind_is_api_writable), so an arm
        # reorder is not a drift.
        "relation": "set_equal",
        "why": "parse_kind is the only gate on both cart write paths (cart_persistence.t27:"
               "34-41); bike_rental unwritable, no deal kind in a cart row and zero kinds "
               "that can write today all turn on this arm list staying these four",
    },
    # Added 2026-09-26 with the kept-cart change: a cart kept from the previous shop is stored and
    # never served. The contract's list of the kinds a cart serves IS the code's list, the one the
    # cart API, the reminder and the Mini App all ask through trios::pricing::cart_kind_is_served.
    # The witness pins the predicate to that array, so a literal of its own inside the predicate
    # (`kind == "bike_rental" || kind == "tea"`) cannot leave this row green. Planted RED once by
    # hand: "tea" added to the array, and the predicate rewritten to compare a literal.
    {
        "name": "cart_persistence.SERVED_CART_KINDS ~ trios/pricing.rs SERVED_CART_KINDS",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "SERVED_CART_KINDS",
        "source": "src/trios/pricing.rs",
        "extract": ("regex_list", r"pub const SERVED_CART_KINDS:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\]\s*;"),
        "witness": r"pub fn cart_kind_is_served\(kind: &str\) -> bool \{\s*SERVED_CART_KINDS\.contains\(&kind\)\s*\}",
        # set_equal: the list is read by membership only (contains), so an order is not a fact.
        "relation": "set_equal",
        "why": "which stored cart lines a customer is shown at all: the owner's rulings of "
               "2026-09-24 and 2026-09-25 (answer 12) leave the rental line and nothing else, and "
               "a kind added here reaches the cart API, the reminder and the Mini App at once",
    },
    # The same change's census of the reminder's reads of cart_items: both carry the served-kind
    # filter, the inner join (witness) included, which is what keeps a cart of hidden rows from ever
    # being due. Planted RED once by hand: the join's filter removed (count 1, witness blind).
    {
        "name": "cart_persistence.REMINDER_KIND_FILTERED_QUERIES ~ cart_abandonment.rs kind filters",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "REMINDER_KIND_FILTERED_QUERIES",
        "source": "src/cart_abandonment.rs",
        "extract": ("regex_count", r"kind = ANY\(\$\d\)"),
        "witness": r"JOIN cart_items ci ON ci\.cart_id = c\.id AND ci\.kind = ANY\(\$3\)",
        "relation": "equal",
        "why": "the reminder sums, names and spends a rung on the lines these two queries return; "
               "a read without the filter would put a hidden line's name into a Telegram message",
    },
    {
        "name": "cart_persistence.CARTS_COLUMNS ~ cart entity fields",
        "spec": "specs/turbobaby/cart_persistence.t27",
        "const": "CARTS_COLUMNS",
        "source": "src/db/entities/cart.rs",
        # Every `name: Type` field of Model, pub or not, so dropping `pub` cannot hide a
        # field. `:\s` skips `::` paths. Measured 10, the file's only struct.
        "extract": ("regex_all", r"^\s*(?:pub\s+)?(\w+):\s"),
        # set_equal: the contract names these as a set of columns and indexes nothing, so a
        # field reorder in the entity is not a drift.
        "relation": "set_equal",
        "why": "cart_persistence.t27:11-16 counts ten columns across 059/061/062 and says the "
               "entity lists the same ten, no status among them; a field added to carry LOCKED "
               "or ABANDONED makes the no-representation verdicts wrong. A migration-only "
               "column the entity never learns about is outside this row",
    },
    # The next four read data/fleet_seed.json, the first JSON source in this table. It is
    # delivery_terms.t27's declared SOURCE, "the repository-tracked file a gate could bind
    # to" (delivery_terms.t27:21-26); read_text and the tracked check treat it like any
    # other file. Measured 2026-09-22: no file under src/ reads the seed, and outside
    # specs/ the four keys read below (known_tariff_example, delivery_thb, pickup_thb,
    # default_point) occur in the seed alone -- these rows keep the contract and its
    # provenance record in step, not a served price.
    {
        "name": "delivery_terms.DOCUMENTED_DELIVERY_FEE_THB ~ seed known_tariff_example",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "DOCUMENTED_DELIVERY_FEE_THB",
        "source": "data/fleet_seed.json",
        # `[0-9.]+` then a JSON delimiter, so 290.5 is read as 290.5 and not as 290.
        "extract": ("regex", r'"known_tariff_example":\s*\{[^}]*"delivery_thb":\s*([0-9.]+)\s*[,}]'),
        "relation": "equal",
        "why": "the one delivery price in this repository; the contract pins its ladder's "
               "first rung to it (delivery_terms.t27, test "
               "the_ladder_replaced_a_one_row_table_and_the_old_reading_is_kept: "
               "LADDER_DISTRICT_FEE_THB[0] == DOCUMENTED_DELIVERY_FEE_THB), so a re-priced "
               "seed under a stale contract leaves the repository stating two fees for one "
               "district",
    },
    {
        "name": "delivery_terms.DOCUMENTED_COLLECTION_FEE_THB ~ seed known_tariff_example pickup",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "DOCUMENTED_COLLECTION_FEE_THB",
        "source": "data/fleet_seed.json",
        "extract": ("regex", r'"known_tariff_example":\s*\{[^}]*"pickup_thb":\s*([0-9.]+)\s*[,}]'),
        # Expected value is ZERO (D16): the witness proves the tariff object and its pickup
        # key are still there to be read. It does not lean on the delivery fee, so a seed
        # that made delivery free reddens the delivery row, not this one.
        "witness": r'"known_tariff_example":\s*\{[^}]*"pickup_thb":',
        "relation": "equal",
        "why": "the contract's one legitimate zero -- collection is free -- and the value "
               "FEE_ABSENT must never be read as; a seed that starts charging for collection "
               "makes an_absent_fee_is_never_the_published_zero argue from a false zero",
    },
    {
        "name": "delivery_terms.DOCUMENTED_AREA_NAMES ~ seed known_tariff_example area",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "DOCUMENTED_AREA_NAMES",
        "source": "data/fleet_seed.json",
        # `[^}]*` rather than `\s*`: the area is found wherever it sits inside the object,
        # so a key reorder in the seed is not a drift.
        "extract": ("regex_all", r'"known_tariff_example":\s*\{[^}]*"area":\s*"([^"]+)"'),
        "relation": "list_equal",
        "why": "which district the documented fee belongs to; the fee row cannot see a seed "
               "that moves 290 to another area, and the contract asserts "
               "LADDER_DISTRICT_NAMES[0] == DOCUMENTED_AREA_NAMES[0]",
    },
    {
        "name": "delivery_terms.DEFAULT_POINT ~ seed delivery.default_point",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "DEFAULT_POINT",
        "source": "data/fleet_seed.json",
        "extract": ("regex", r'"default_point":\s*("[^"]*")'),
        "relation": "equal",
        "why": "where a customer collects the bike when nothing else is agreed; "
               "handover_point_is_settled treats this point as settled without agreement",
    },
    # --- delivery zones group: the owner's Phuket table (owner, 2026-09-24) -------------------
    # Added 2026-09-24 with migrations/087_delivery_zones_phuket.sql. The contract's zone table
    # (delivery_terms.PHUKET_ZONE_*) is bound to BOTH places the table ships -- the migration that
    # inserts the rows and the built-in copy in src/delivery.rs -- by count, by name and order,
    # and by three fees that are each an owner decision (Pa Khlok 490 and Mai Khao 990 of
    # 2026-09-06, the airport's 690 of 2026-09-24). The ETA rows count what must stay ZERO, each
    # with a witness (D16). Every row was measured by hand on both sides before it was written.
    {
        "name": "delivery_terms.PHUKET_ZONE_COUNT ~ 087 inserted rows",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PHUKET_ZONE_COUNT",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex_count", PHUKET_ROW_SQL),
        "relation": "equal",
        "why": "the owner decided eighteen zones; a row added to or dropped from the migration "
               "is a zone customers can or cannot pick that the contract does not know about",
    },
    {
        "name": "delivery_terms.PHUKET_ZONE_NAMES_EN ~ 087 name_en column",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PHUKET_ZONE_NAMES_EN",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex_all", PHUKET_ROW_SQL_NAME),
        "relation": "list_equal",
        "why": "names AND order: the sort orders follow the list, and the contract's fee, source "
               "and ladder-index arrays are indexed by the same position",
    },
    {
        "name": "delivery_terms.PAKLOK_FEE_THB ~ 087 Pa Khlok fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PAKLOK_FEE_THB",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex", r"^[ \t]+\('[^'\n]+',[ \t]*'Pa Khlok',[ \t]*([0-9.]+),"),
        "relation": "equal",
        "why": "owner decision 2026-09-06-3 moved Paklok 390 -> 490; a migration carrying the "
               "superseded 390 would ship the older price as current",
    },
    {
        "name": "delivery_terms.MAIKHAO_FEE_THB ~ 087 Mai Khao fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "MAIKHAO_FEE_THB",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex", r"^[ \t]+\('[^'\n]+',[ \t]*'Mai Khao',[ \t]*([0-9.]+),"),
        "relation": "equal",
        "why": "owner decision 2026-09-06-4: 990 is a live price, not sheet garbage; a 'tidied' "
               "neighbour price here is exactly what that decision forbids",
    },
    {
        "name": "delivery_terms.AIRPORT_ZONE_FEE_THB ~ 087 Airport fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "AIRPORT_ZONE_FEE_THB",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex", r"^[ \t]+\('[^'\n]+',[ \t]*'Airport',[ \t]*([0-9.]+),"),
        "relation": "equal",
        "why": "the owner chose 690 from the published 590-690 range on 2026-09-24; a midpoint "
               "or the lower end here would be a price nobody chose",
    },
    {
        "name": "delivery_terms.OTHER_ISLAND_ZONE_NAMES_EN ~ 087 deactivation list",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "OTHER_ISLAND_ZONE_NAMES_EN",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex_list", r"WHERE name_en IN \(([^)]*)\)"),
        "relation": "list_equal",
        "why": "the four Koh Phangan rows 087 deactivates (and deletes none of); a village missing "
               "from this list stays in the customer's picker",
    },
    {
        "name": "delivery_terms.ETA_COLUMNS_MADE_NULLABLE_BY_087 ~ 087 DROP NOT NULL",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "ETA_COLUMNS_MADE_NULLABLE_BY_087",
        "source": "migrations/087_delivery_zones_phuket.sql",
        "extract": ("regex_count", r"^ALTER TABLE delivery_zones ALTER COLUMN eta_(?:min|max) DROP NOT NULL;"),
        "relation": "equal",
        "why": "an ETA column left NOT NULL can only hold a number, which is how 060 put 30 and "
               "60 minutes on every row nobody had measured",
    },
    {
        "name": "delivery_terms.PHUKET_ZONE_COUNT ~ delivery.rs built-in rows",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PHUKET_ZONE_COUNT",
        "source": "src/delivery.rs",
        "extract": ("regex_count", PHUKET_ROW_RS),
        "relation": "equal",
        "why": "the built-in copy mirrors migration 087; two tables of different lengths is the "
               "state 071 was written to end",
    },
    {
        "name": "delivery_terms.PHUKET_ZONE_NAMES_EN ~ delivery.rs built-in name_en",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PHUKET_ZONE_NAMES_EN",
        "source": "src/delivery.rs",
        "extract": ("regex_all", PHUKET_ROW_RS_NAME),
        "relation": "list_equal",
        "why": "same names in the same order as the migration, through the contract; a zone "
               "renamed in one copy only is two zones",
    },
    {
        "name": "delivery_terms.PAKLOK_FEE_THB ~ delivery.rs Pa Khlok fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "PAKLOK_FEE_THB",
        "source": "src/delivery.rs",
        "extract": ("regex", r'^[ \t]+\("pa_khlok",[^\n]*,[ \t]*([0-9.]+)\),'),
        "relation": "equal",
        "why": "the built-in copy of an owner decision; see the migration row",
    },
    {
        "name": "delivery_terms.MAIKHAO_FEE_THB ~ delivery.rs Mai Khao fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "MAIKHAO_FEE_THB",
        "source": "src/delivery.rs",
        "extract": ("regex", r'^[ \t]+\("mai_khao",[^\n]*,[ \t]*([0-9.]+)\),'),
        "relation": "equal",
        "why": "the built-in copy of an owner decision; see the migration row",
    },
    {
        "name": "delivery_terms.AIRPORT_ZONE_FEE_THB ~ delivery.rs Airport fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "AIRPORT_ZONE_FEE_THB",
        "source": "src/delivery.rs",
        "extract": ("regex", r'^[ \t]+\("airport",[^\n]*,[ \t]*([0-9.]+)\),'),
        "relation": "equal",
        "why": "the built-in copy of the owner's choice from a range; see the migration row",
    },
    {
        "name": "delivery_terms.SHIPPED_HANDOVER_ROW_FEE_THB ~ delivery.rs pickup fee",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "SHIPPED_HANDOVER_ROW_FEE_THB",
        "source": "src/delivery.rs",
        "extract": ("regex", r'"Pickup at TurboBaby",\s*([0-9.]+),'),
        # Expected value is ZERO (D16): the witness proves the pickup row is still the
        # constant the regex reads.
        "witness": r"const PICKUP_ZONE: ZoneRow",
        "relation": "equal",
        "why": "collecting at the shop's own counter costs nothing; a fee here would charge a "
               "customer for walking in",
    },
    {
        "name": "delivery_terms.BUILT_IN_ZONES_WITH_AN_ETA ~ delivery.rs absence",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "BUILT_IN_ZONES_WITH_AN_ETA",
        "source": "src/delivery.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"_eta_minutes:\s*Some\("),
        # Zero expected (D16): the field is still declared, so the scan can see where a
        # Some would go.
        "witness": r"pub min_eta_minutes: Option<u32>,",
        "relation": "equal",
        "why": "no ETA is published (owner, 2026-09-24); a Some here is a travel time nobody "
               "measured, which is what the five Koh Phangan defaults carried",
    },
    {
        "name": "delivery_terms.CHECKOUT_ETA_FALLBACK_LITERALS ~ checkout_screen.rs absence",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "CHECKOUT_ETA_FALLBACK_LITERALS",
        "source": "src/ui/screens/checkout_screen.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"30-45"),
        # Zero expected (D16): the fee line the checkout keeps is still there to be read.
        "witness": r"T_DELIVERY_FEE,",
        "relation": "equal",
        "why": "the checkout printed a hard-coded 30-45 minute ETA when the zone list was "
               "empty; no source measures it and the shop publishes windows, not minutes",
    },
    {
        "name": "delivery_terms.SUCCESS_SCREEN_ETA_FALLBACK_LITERALS ~ success_screen.rs absence",
        "spec": "specs/turbobaby/delivery_terms.t27",
        "const": "SUCCESS_SCREEN_ETA_FALLBACK_LITERALS",
        "source": "src/ui/screens/success_screen.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"30-45"),
        # Zero expected (D16): the ETA row's value key is still there to be read.
        "witness": r"T_SUCCESS_ETA_VALUE",
        "relation": "equal",
        "why": "the success screen fell back to the same literal after the order was placed; "
               "an absent ETA renders no row",
    },
    # --- catalog group: validation bounds, catalog api, http cache, rental terms, happy hour ---
    # Added 2026-09-22. Each row was measured both sides by hand at a33e500 and went RED for at
    # least one drift its `why` names: a file row through --source-override on a planted copy,
    # a tree_* row (a glob, which cannot be overridden) by running this script from a mirror of
    # the tracked tree with one file planted in it. Not for every drift a `why` names: review
    # the same day found the rental_terms rows green on a half-applied re-cut of a band's END
    # (week max_days 13 -> 14 with two_weeks still starting at 14), since they read min_days
    # only. The two band-end rows and the month row's witness below were planted RED on it.
    # Line citations into validation_bounds.t27, catalog_api.t27 and rental_terms.t27 are
    # taken AFTER this change's corrections to them.
    {
        "name": "validation_bounds.ID_MAX_SAFE ~ auth.rs validate_telegram_id_param",
        "spec": "specs/turbobaby/validation_bounds.t27",
        "const": "ID_MAX_SAFE",
        "source": "src/api/auth.rs",
        # Anchored on the function, because `if id >` alone is a shape any validator
        # can take; the floor check in front of it is part of the anchor so the
        # capture cannot slide into a neighbouring function.
        "extract": ("regex", r"fn validate_telegram_id_param\(id: i64\)[^{]*\{\s*if id <= 0 \{[^}]*\}\s*(?://[^\n]*\s*)?if id > ([0-9_]+)\s*\{"),
        "relation": "equal",
        "why": "the ceiling every Telegram id in a path or query meets on 30 call sites in "
               "10 modules (validation_bounds.t27:534-541); a drift either 400s real "
               "customers everywhere or admits ids a JS client cannot hold exactly",
    },
    {
        "name": "validation_bounds.ID_MAX_SAFE ~ validation.rs validate_telegram_id",
        "spec": "specs/turbobaby/validation_bounds.t27",
        "const": "ID_MAX_SAFE",
        "source": "src/trios/validation.rs",
        # The contract names both homes of this bound (the second-copy paragraph), which is
        # what licenses a second row for one (contract, constant). Under the constant it
        # cites, validation_bounds.t27 now records (2026-09-22) that tests/t27_gates_run.rs
        # runs this gate, so the pin reaches cargo test through ID_MAX_SAFE; no test
        # compares the two literals with each other.
        "extract": ("regex", r"pub fn validate_telegram_id\(id: i64\)[^{]*\{\s*if id <= 0 \{[^}]*\}\s*(?://[^\n]*\s*)?if id > ([0-9_]+)\s*\{"),
        "relation": "equal",
        "why": "the second copy of the same bound; with the row above, the gate is what "
               "pins the two copies equal; validation_bounds.t27:544-561 records that as a "
               "correction (A_TEST_PINS_THE_TWO_COPIES_EQUAL true from 2026-09-22, false the "
               "day before)",
    },
    {
        "name": "validation_bounds.LIVE_TOKEN_MAX_CHARS ~ quest.rs extract_qr_token",
        "spec": "specs/turbobaby/validation_bounds.t27",
        "const": "LIVE_TOKEN_MAX_CHARS",
        "source": "src/api/quest.rs",
        # `len() > 200` alone matches 12 times in this file; the function is the anchor,
        # and `[^}]*?` keeps the capture inside its body.
        "extract": ("regex", r"fn extract_qr_token\([^)]*\)[^{]*\{[^}]*?if token\.len\(\) > (\d+) \|\| token\.is_empty\(\)"),
        "relation": "equal",
        "why": "the only length rule the live QR scan applies before the lookup "
               "(validation_bounds.t27:452-456) -- the strict 16-char grammar runs "
               "nowhere, so this number alone decides which scans reach the database",
    },
    {
        "name": "validation_bounds.LIVE_TOKEN_BODY_KEYS ~ quest.rs extract_qr_token",
        "spec": "specs/turbobaby/validation_bounds.t27",
        "const": "LIVE_TOKEN_BODY_KEYS",
        "source": "src/api/quest.rs",
        # File order IS precedence here: the second key is read through `.or(`.
        "extract": ("regex_all", r"(?:let token = body|\.or\(body)\[\"([a-z_]+)\"\]"),
        "relation": "list_equal",
        "why": "the two body keys a scan is accepted under and their precedence "
               "(validation_bounds.t27:458-460); a renamed or reordered key turns a "
               "client's scan into a 400 or reads the wrong field first",
    },
    {
        "name": "catalog_api.FILTER_NAMES ~ bikes.rs query words read",
        "spec": "specs/turbobaby/catalog_api.t27",
        "const": "FILTER_NAMES",
        "source": "src/api/bikes.rs",
        # The three reads CatalogFilter::parse makes (q.get("class"), cc("min_cc"),
        # cc("max_cc")) plus the separate available_only flag in list_bikes. Tests call
        # query_flag with `&q(&[...])`, which `&q,` does not match. set_equal, because
        # position 3 is pinned inside the contract by its own invariant and the source's
        # file order is not a claim.
        "extract": ("regex_all", r"(?:\bq\.get\(|\bcc\(|query_flag\(&q,\s*)\"([a-z_]+)\"\)"),
        "relation": "set_equal",
        "why": "unknown query keys are dropped under a 200 (catalog_api.t27:35-44), so a "
               "filter word the contract publishes and the parser does not read is a "
               "silently WIDER catalog -- the cc_min/min_cc drift this file corrected",
    },
    {
        "name": "catalog_api.UNIT_ROLLUP_QUERY_ROW_CAP ~ bikes.rs UNIT_QUERY_LIMIT",
        "spec": "specs/turbobaby/catalog_api.t27",
        "const": "UNIT_ROLLUP_QUERY_ROW_CAP",
        "source": "src/api/bikes.rs",
        "extract": ("regex", r"const UNIT_QUERY_LIMIT:\s*u64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the row cap on one family's unit rollup behind GET /api/bikes/:key "
               "(catalog_api.t27:401-405, :397-401 until 2026-09-26); lowered under a family's unit count it would "
               "under-report that family's units with no error anywhere",
    },
    {
        "name": "catalog_api.CURSOR_OCCURRENCES_IN_THE_API_MODULE ~ src/api absence",
        "spec": "specs/turbobaby/catalog_api.t27",
        "const": "CURSOR_OCCURRENCES_IN_THE_API_MODULE",
        "source": "src/api/**/*.rs",
        # Case-sensitive, because that is the grep catalog_api.t27:325 took: a
        # std::io::Cursor in an upload or export handler is not pagination and must not
        # turn this red. A cursor parser reads a lowercase `cursor` key.
        "extract": ("tree_regex_count", r"cursor"),
        # Expected value is zero (D16): the scan must be shown to see Rust at all.
        "witness": r"fn \w+\(",
        "relation": "equal",
        "why": "catalog_api.t27:321-332 marks the page and cursor budgets as binding "
               "nothing because nothing in src/api/ parses a cursor; the day one does, "
               "PAGINATION_TRANSPORT_IS_NOT_SHIPPED is false and this row says so",
    },
    {
        "name": "http_cache.CATALOG_MAX_AGE_SECONDS ~ bikes.rs list cache-control",
        "spec": "specs/turbobaby/http_cache.t27",
        "const": "CATALOG_MAX_AGE_SECONDS",
        "source": "src/api/bikes.rs",
        # The WHOLE directive is the anchor, so adding must-revalidate or no-cache (both
        # declared false at http_cache.t27:376-377) also turns this row red.
        "extract": ("regex", r"HeaderValue::from_static\(\"public, max-age=(\d+)\"\)"),
        "relation": "equal",
        "why": "how long a client may act on a catalog body -- prices and unit counts -- "
               "without asking (http_cache.t27:370-377); the whole stale-price window "
               "the contract reasons about is this one number",
    },
    {
        "name": "http_cache.BODY_SOURCE_FIELD_VALUES ~ bikes.rs CLIENT_RATE_* consts",
        "spec": "specs/turbobaby/http_cache.t27",
        "const": "BODY_SOURCE_FIELD_VALUES",
        "source": "src/api/bikes.rs",
        # Any visibility, so a third value declared `pub(crate) const` is still counted;
        # set_equal, because the two declarations' file order is not a claim. A value
        # emitted as an inline literal instead of through a CLIENT_RATE_ const is not seen.
        "extract": ("regex_all", r"^(?:pub(?:\([^)]*\))?\s+)?const CLIENT_RATE_[A-Z_]+:\s*&(?:'static\s+)?str\s*=\s*\"([^\"]*)\"\s*;"),
        "relation": "set_equal",
        "why": "the provenance words a served price carries (http_cache.t27:677-688); "
               "a third value such as 'not reconciled' is exactly what flips "
               "RULE_NEEDS_A_SOURCE_VALUE_THAT_DOES_NOT_EXIST_YET, and must not ship unseen",
    },
    {
        "name": "http_cache.INVALIDATORS_NAMING_A_LIVE_KEY ~ cache.rs absence",
        "spec": "specs/turbobaby/http_cache.t27",
        "const": "INVALIDATORS_NAMING_A_LIVE_KEY",
        "source": "src/api/cache.rs",
        # Zero expected (D16). The map field is private to this module, so any
        # invalidator lives here. Counted: a remove whose key is a "bikes..." literal OR
        # is not a literal at all (a generic invalidate(key) called with a live key from
        # elsewhere), a prefix sweep, and the three whole-map shapes. The five removals
        # of dead key names are string literals and do not match.
        "extract": ("regex_count", r"\.remove\((?:\"bikes|(?!\"))|starts_with\(\"bikes|\.clear\(\)|\.retain\(|\.drain\("),
        "witness": r"\.remove\(\"[a-z_]+\"\)",
        "relation": "equal",
        "why": "http_cache.t27:298-311 says no invalidator touches a live catalog key, and "
               ":395-398 that removing one would recall no copy a client already holds; "
               "an edit that starts clearing catalog keys should meet both paragraphs first",
    },
    {
        "name": "http_cache.DECLARED_REPLICAS ~ railway.toml numReplicas",
        "spec": "specs/turbobaby/http_cache.t27",
        "const": "DECLARED_REPLICAS",
        # Not .rs or .sql: the deployment manifest is where the fact lives.
        "source": "railway.toml",
        # One match is also the one-region claim: a second region adds a second match.
        "extract": ("regex", r"numReplicas\s*=\s*(\d+)"),
        "relation": "equal",
        "why": "the map is coherent only because one process holds it; "
               "http_cache.t27:798 says this can change 'in one edit to a file no test "
               "in this repository reads', and this row is that reader",
    },
    # The six rows on migrations/079_rental_terms.sql share the limit stated on
    # DAYS_PER_FORTNIGHT: 079 runs once, so a LATER migration that re-cuts a band is unseen.
    # Three read band STARTS, two read band ENDS (added 2026-09-22, after review planted a
    # half-applied re-cut -- week max_days 13 -> 14, two_weeks still from 14 -- and every
    # start row stayed green), and the month row's witness pins the month's open end.
    {
        "name": "rental_terms.DAYS_PER_WEEK ~ 079 week band min_days",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "DAYS_PER_WEEK",
        "source": "migrations/079_rental_terms.sql",
        "extract": ("regex", r"\('week',\s*(\d+),"),
        "relation": "equal",
        "why": "below this day no band exists (BAND_NONE, D9); the served first band "
               "must start where the contract says absence ends",
    },
    {
        "name": "rental_terms.DAYS_PER_FORTNIGHT ~ 079 two_weeks band min_days",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "DAYS_PER_FORTNIGHT",
        "source": "migrations/079_rental_terms.sql",
        # 079 is the only migration that writes rental_terms (measured 2026-09-22), and
        # src/db/mod.rs runs each migration exactly once: an in-place edit here reaches a
        # fresh database only, and a LATER migration that re-cuts the band is outside this
        # row altogether.
        "extract": ("regex", r"\('two_weeks',\s*(\d+),"),
        "relation": "equal",
        "why": "band_index_for_days starts two_weeks at this day and 079 seeds the same "
               "min_days that GET /api/rental-terms serves; the owner's 14-vs-15 cut is "
               "unmapped (rental_terms.t27 TERM_BUCKET_BOUNDARY_IS_MAPPED = false), so "
               "re-cutting it in the contract must meet the seeded table. A later migration "
               "re-cutting the band is not seen",
    },
    {
        "name": "rental_terms.DAYS_PER_MONTH ~ 079 month band min_days",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "DAYS_PER_MONTH",
        "source": "migrations/079_rental_terms.sql",
        "extract": ("regex", r"\('month',\s*(\d+),"),
        # The month band's END is NULL::INT (open-ended), which the contract declares as
        # MONTH_BAND_HAS_A_LAST_DAY = false and a bool cannot be bound. The witness pins it:
        # a capped month band (30..179) makes band_index_for_days' "a month or longer"
        # reading wrong past the cap, and was planted RED on 2026-09-22.
        "witness": r"\('month',\s*\d+,\s*NULL::INT,",
        "relation": "equal",
        # Tightened on review: the 0.10 gap is on the DISCOUNT axis, not the day axis.
        "why": "the day the month band starts in the contract's reading and in the "
               "served table; it is also the day the discount jumps across the 0.10 gap "
               "between two_weeks' 0.25 ceiling and month's 0.35 floor, which the tariff "
               "publishes nothing for",
    },
    {
        "name": "rental_terms.WEEK_BAND_LAST_DAY ~ 079 week band max_days",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "WEEK_BAND_LAST_DAY",
        "source": "migrations/079_rental_terms.sql",
        "extract": ("regex", r"\('week',\s*\d+,\s*(\d+),"),
        "relation": "equal",
        "why": "the served week band ends here and the contract asserts it ends the day before "
               "DAYS_PER_FORTNIGHT; with only the starts bound, week max_days moved to 14 while "
               "two_weeks still starts at 14 served two bands on one day and stayed green",
    },
    {
        "name": "rental_terms.TWO_WEEKS_BAND_LAST_DAY ~ 079 two_weeks band max_days",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "TWO_WEEKS_BAND_LAST_DAY",
        "source": "migrations/079_rental_terms.sql",
        "extract": ("regex", r"\('two_weeks',\s*\d+,\s*(\d+),"),
        "relation": "equal",
        "why": "the served two_weeks band ends here and the contract asserts it ends the day "
               "before DAYS_PER_MONTH; moved on its own it opens a gap or an overlap at the "
               "month seam that no start row sees",
    },
    {
        "name": "rental_terms.BAND_NAMES ~ 079 rental_terms rows",
        "spec": "specs/turbobaby/rental_terms.t27",
        "const": "BAND_NAMES",
        "source": "migrations/079_rental_terms.sql",
        # set_equal: the SERVED order is list_rental_term_bands' order_by_asc(MinDays)
        # (src/db/bikes.rs:513), not the order of these VALUES rows, and the three
        # DAYS_PER_* rows already pin each band's min_days by name. Reordering the rows
        # changes nothing a client sees and must not turn this red.
        "extract": ("regex_all", r"\('([a-z_]+)',\s*\d+,\s*(?:\d+|NULL::INT),\s*[0-9.]+,\s*[0-9.]+\)"),
        "relation": "set_equal",
        "why": "band_index_for_days returns an INDEX into this array and the served "
               "term_bands are these rows sorted by min_days; a renamed, added or dropped "
               "band breaks that index, and the DAYS_PER_* rows pin the order",
    },
    {
        "name": "happy_hour.FALLBACK_START_HOUR ~ happy_hour.rs reader and no-row arm",
        "spec": "specs/turbobaby/happy_hour.t27",
        "const": "FALLBACK_START_HOUR",
        "source": "src/api/happy_hour.rs",
        # happy_hour.t27:288 declares the pair twice: the unwrap_or in the reader and the
        # no-row arm. Both copies must carry the same number or this row is red.
        "extract": ("regex", r"(?:\bhappy_hour\[\"start\"\]\.as_i64\(\)\.unwrap_or\(|None => Ok\(Json\(json!\(\{[^}]*?\"start\":\s*)(\d+)"),
        "occurrences": 2,
        "relation": "equal",
        "why": "the start hour GET /api/happy-hour actually publishes, because the shipped "
               "blob holds none of the reader's keys (happy_hour.t27:9-18); a window "
               "whose only source is a fallback literal must not move unseen",
    },
    {
        "name": "happy_hour.FALLBACK_END_HOUR ~ happy_hour.rs reader and no-row arm",
        "spec": "specs/turbobaby/happy_hour.t27",
        "const": "FALLBACK_END_HOUR",
        "source": "src/api/happy_hour.rs",
        "extract": ("regex", r"(?:\bhappy_hour\[\"end\"\]\.as_i64\(\)\.unwrap_or\(|None => Ok\(Json\(json!\(\{[^}]*?\"end\":\s*)(\d+)"),
        "occurrences": 2,
        "relation": "equal",
        "why": "same served window, the exclusive end; the copies at happy_hour.rs:41 and :84 "
               "have no test holding them equal",
    },
    {
        "name": "happy_hour.READER_KEYS_FOUND_IN_SHIPPED_BLOB ~ migrations absence",
        "spec": "specs/turbobaby/happy_hour.t27",
        "const": "READER_KEYS_FOUND_IN_SHIPPED_BLOB",
        "source": "migrations/**/*.sql",
        # A nested happy_hour object in any migration, as a JSON key, a jsonb path, or a
        # quoted key. Zero expected, so the flat seeded keys stand witness (D16).
        "extract": ("tree_regex_count", r"\"happy_hour\"\s*:|'\{happy_hour\b|'happy_hour'"),
        "witness": r"\"happy_hour_start\"\s*:",
        "relation": "equal",
        "why": "the headline of happy_hour.t27:262-280: no shipped config holds the shape "
               "the reader wants, so the served window is the fallback; a migration that "
               "ships the nested shape changes what customers are told",
    },
    {
        "name": "happy_hour.HTTP_CONSUMERS_IN_REPO ~ src/ui absence",
        "spec": "specs/turbobaby/happy_hour.t27",
        "const": "HTTP_CONSUMERS_IN_REPO",
        "source": "src/ui/**/*.rs",
        "extract": ("tree_regex_count", r"(?i)happy[-_ ]?hour"),
        # Zero expected (D16): the client's /api paths must be visible to the scan.
        "witness": r"\"/api/",
        "relation": "equal",
        "why": "DISCOUNT_REACHES_A_PRICE = false and 'nothing in this tree renders it' "
               "(happy_hour.t27:342-357) rest on this zero; the first src/ui fetch or "
               "banner puts the source-less 18-21 fallback in front of a customer. Covers "
               "src/ui only: assets/ and e2e/, which the contract also names, are not scanned",
    },
    # --- reach group: bot surface, ai assist, notification queue, promo broadcast, deeplink ---
    # Added 2026-09-22. Each row was measured both sides by hand at a33e500 and went RED for at
    # least one drift its `why` names: a file row through --source-override on a planted copy,
    # a tree_* row by running this script from a mirror of the tracked tree with one file
    # planted in it. Not for every drift a `why` names: review the same day found four count
    # rows green on the gate or call COMMENTED OUT (ADMIN_GATED_COMMAND_COUNT,
    # SITES_WITH_A_LIMITER, CALL_SITE_COUNT, DRAINER_SPAWN_SITES) and ESCAPED_CHARS green on
    # two replacements swapped. The four now carry NOT_IN_A_LINE_COMMENT and ESCAPE_REPLACEMENTS
    # has its own row, each planted RED on exactly that drift. Fixing them turned up one more of
    # the same class the review had not planted, INJECTION_MARKER_COUNT (its first marker
    # commented out read 25); it is comment-aware now and was planted RED. Citations into
    # ai_assist.t27 and deeplink.t27 are taken AFTER this change's dated notes in them.
    {
        "name": "bot_surface.ADMIN_GATED_COMMAND_COUNT ~ commands.rs roster gate lines",
        "spec": "specs/turbobaby/bot_surface.t27",
        "const": "ADMIN_GATED_COMMAND_COUNT",
        "source": "src/bot/commands.rs",
        # bot_surface.t27:214-216 says the gate is "the same single line" six times. The
        # callback gate in src/bot/callbacks.rs:227 has a different shape
        # (`action.requires_admin() && ...`) and a different file, so it cannot answer here.
        # A count and not a map: a gate MOVED from one arm to another keeps it at 6 and stays
        # green (measured 2026-09-22 on a planted copy); a gate deleted goes red, and since the
        # same day so does a gate commented out (NOT_IN_A_LINE_COMMENT; it was green before).
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"if !config\.admin_ids\.contains\(&user_id\)"),
        "relation": "equal",
        "why": "six commands (engage, factpost, unblock, blocks, errors, promo) are refused to "
               "anyone off the roster by exactly this line; deleting one copy opens an admin "
               "command -- /unblock or /promo -- to every customer and nothing else notices",
    },
    {
        "name": "bot_surface.REFERRAL_PREFIX ~ commands.rs /start referral literal",
        "spec": "specs/turbobaby/bot_surface.t27",
        "const": "REFERRAL_PREFIX",
        "source": "src/bot/commands.rs",
        # The literal is spelled twice -- starts_with and trim_start_matches -- and a
        # half-edit would recognise one prefix and strip another. Both must match. Reader
        # side only: the invite-link BUILDERS (src/api/referrals.rs, the /invite arm of this
        # file, src/ui/screens/profile_screen.rs) spell the prefix inside format! strings,
        # which no extractor here reads as a str, so a builder-only rename is not seen.
        "extract": ("regex", r"args\.(?:starts_with|trim_start_matches)\((\"[^\"]*\")\)"),
        "occurrences": 2,
        "relation": "equal",
        "why": "every invite link already sitting in a customer's Telegram history carries this "
               "prefix; renaming it stops referrals being recorded with no error anywhere "
               "(bot_surface.t27:303-308: an unparseable referral is silent)",
    },
    {
        "name": "bot_surface.CALLBACK_PREFIXES ~ route_callback prefix arms",
        "spec": "specs/turbobaby/bot_surface.t27",
        "const": "CALLBACK_PREFIXES",
        "source": "src/bot/callbacks.rs",
        # The file's test module calls data.starts_with("sotd_next_") too (:968, a 7th hit
        # for the bare call). The leading `if` and the `return CallbackAction::` tail each
        # exclude it on their own (6 with either, measured 2026-09-22); the tail is what
        # ties the capture to route_callback's arms. The six are disjoint today, so a
        # reorder cannot misroute and is only a red against the contract's declared
        # sequence; the row exists for a RENAMED or dropped arm. The builders (src/api/orders.rs:1774-1775, this file :546) spell
        # the same prefixes inside format! strings and are not read here.
        "extract": ("regex_all", r"if (?:let Some\(\w+\) = )?data\.(?:strip_prefix|starts_with)\(\"([^\"]*)\"\)\s*\{\s*return CallbackAction::"),
        "relation": "list_equal",
        "why": "the last three prefixes carry an order id to the admin-only order actions; a "
               "renamed or dropped arm turns every such button already sitting in an admin's "
               "chat into Unknown, which answers the press, does nothing and leaves only a warn "
               "line in the log (bot_surface.t27:324-325 pins the six and their sequence)",
    },
    {
        "name": "bot_surface.ESCAPED_CHARS ~ util.rs html_escape order",
        "spec": "specs/turbobaby/bot_surface.t27",
        "const": "ESCAPED_CHARS",
        "source": "src/util.rs",
        # Captures the CHARACTER of each .replace, in call order. The replacement strings are
        # read by the next row: a swapped pair stays green HERE (planted 2026-09-22) and is
        # red there.
        "extract": ("regex_all", r"\.replace\('(.)',\s*\"&\w+;\"\)"),
        "relation": "list_equal",
        "why": "the order is the whole correctness argument (bot_surface.t27:425-429): the "
               "ampersand must be replaced first or every < ships double-encoded, and a "
               "parse-mode failure loses the WHOLE message, not a character",
    },
    {
        "name": "bot_surface.ESCAPE_REPLACEMENTS ~ util.rs html_escape replacements",
        "spec": "specs/turbobaby/bot_surface.t27",
        "const": "ESCAPE_REPLACEMENTS",
        "source": "src/util.rs",
        # Added 2026-09-22, the sibling of the row above over the same three calls: the
        # REPLACEMENT of each .replace, in call order. Measured ['&amp;', '&lt;', '&gt;'].
        # Planted RED: '<' -> "&gt;" and '>' -> "&lt;", which the row above does not see.
        "extract": ("regex_all", r"\.replace\('.',\s*\"(&\w+;)\"\)"),
        "relation": "list_equal",
        "why": "the escape is correct only if each character becomes ITS entity; a swapped pair "
               "turns every < in model or customer text into > and back, silently, and the "
               "contract's order argument (bot_surface.t27:425-429) assumes the three are right",
    },
    {
        "name": "ai_assist.SITES_WITH_A_LIMITER ~ src/ census",
        "spec": "specs/turbobaby/ai_assist.t27",
        "const": "SITES_WITH_A_LIMITER",
        "source": "src/**/*.rs",
        # Counts gate LINES, not files. The fitness test at src/bot/mod.rs:286 asks only
        # whether a file containing `.ask_grok(` contains the SUBSTRING ai_rate_limit_allow,
        # and all three calling files carry it in a `// Cycle #129` comment and a `use`
        # line. Measured 2026-09-22: delete all five gates AND the three imports and that
        # test still names no offender. This count goes red on the first gate removed, and
        # since 2026-09-22 on the first gate commented out (NOT_IN_A_LINE_COMMENT: planted
        # green before, RED after) -- the fitness test's own blind spot, which this row had
        # too. It counts limiter lines, so the limiter reused on a non-AI path moves it too.
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT + r"if !ai_rate_limit_allow\(user_id\)"),
        "relation": "equal",
        "why": "each gate is what stands between one customer and unlimited paid-model calls "
               "(denial of wallet), and the fitness test written to hold them "
               "(src/bot/mod.rs:286) is satisfied by a comment: it stays green with every gate "
               "in src/bot deleted",
    },
    {
        "name": "ai_assist.CALL_SITE_COUNT ~ src/ census",
        "spec": "specs/turbobaby/ai_assist.t27",
        "const": "CALL_SITE_COUNT",
        "source": "src/**/*.rs",
        # Excludes the fitness test's quoted `".ask_grok("`, its backticked comment and the
        # `.ask_grok()` prose; the definition has no leading dot. Since 2026-09-22 a call on a
        # commented-out line is not a site either (NOT_IN_A_LINE_COMMENT; planted RED).
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT + r"(?<![`\"])\.ask_grok\((?!\))"),
        "relation": "equal",
        "why": "the six sites are the whole boundary this contract draws; a seventh is a new "
               "path for unreviewed model text to reach a customer, outside the escaping, "
               "limiter and fallback rules pinned here",
    },
    {
        "name": "ai_assist.PROMPT_CHAR_CAP ~ ai.rs MAX_AI_PROMPT_CHARS",
        "spec": "specs/turbobaby/ai_assist.t27",
        "const": "PROMPT_CHAR_CAP",
        "source": "src/ai.rs",
        "extract": ("regex", r"const MAX_AI_PROMPT_CHARS:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the clamp ask_grok applies to the prompt text itself (src/ai.rs:202), so it "
               "holds at all six sites whichever caller sent it; it bounds each attempt's "
               "prompt -- the customer's own words at the free-text site -- not the turn's "
               "cost, which the same contract multiplies by up to seven attempts "
               "(MAX_ATTEMPTS_PER_TURN)",
    },
    {
        "name": "ai_assist.INJECTION_MARKER_COUNT ~ ai.rs denylist",
        "spec": "specs/turbobaby/ai_assist.t27",
        "const": "INJECTION_MARKER_COUNT",
        "source": "src/ai.rs",
        # Counts the string elements of `let dangerous = [...]` only: each must be followed
        # by more elements and then the close that the marker loop reads. A marker REWORDED
        # rather than removed keeps the count, which is all the contract pins. Comment-aware
        # since 2026-09-22 (NOT_IN_A_LINE_COMMENT, and `//` lines may sit in the chain): the
        # FIRST marker commented out left the chain intact and read 25 -- measured on a
        # planted copy, while a later one commented out broke the chain and read 18 -- and
        # both read 24 now. The trailing whitespace moved into the lookahead, because under
        # the line-start prefix a match that ate the newline would skip the next marker.
        "extract": ("regex_count",
                    NOT_IN_A_LINE_COMMENT
                    + r"\"[^\"\n]*\",?(?=\s*(?:(?:\"[^\"\n]*\",?|//[^\n]*)\s*)*\];\s*for marker in dangerous)"),
        "relation": "equal",
        "why": "ai_assist.t27:484-485 pins the count and not the sufficiency; a marker quietly "
               "deleted weakens the one filter in front of the free-text site, where a "
               "customer's own words go into a paid prompt",
    },
    {
        "name": "notification_queue.MAX_ATTEMPTS ~ notification_queue.rs MAX_ATTEMPTS",
        "spec": "specs/turbobaby/notification_queue.t27",
        "const": "MAX_ATTEMPTS",
        "source": "src/notification_queue.rs",
        # The file's own unit test asserts MAX_ATTEMPTS == 3 under cargo; this row is the
        # contract side of the same number and needs no toolchain.
        "extract": ("regex", r"const MAX_ATTEMPTS:\s*i32\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "one declaration feeds both the scan filter and the giveup, so it is at once the "
               "retry budget and the most times one customer can receive the same referral "
               "message (MAX_SENDS_TO_ONE_CUSTOMER_FOR_ONE_ROW)",
    },
    {
        "name": "notification_queue.POLL_INTERVAL_SECONDS ~ POLL_INTERVAL_SECS",
        "spec": "specs/turbobaby/notification_queue.t27",
        "const": "POLL_INTERVAL_SECONDS",
        "source": "src/notification_queue.rs",
        "extract": ("regex", r"const POLL_INTERVAL_SECS:\s*u64\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "with no backoff the gap between retries IS this interval, so the whole retry "
               "horizon (RETRY_HORIZON_SECONDS = 60) and the outage a message survives are "
               "this number times two",
    },
    {
        "name": "notification_queue.DRAINER_SPAWN_SITES ~ src/ census",
        "spec": "specs/turbobaby/notification_queue.t27",
        "const": "DRAINER_SPAWN_SITES",
        "source": "src/**/*.rs",
        # The lookbehind drops the definition at src/notification_queue.rs:24, the same
        # technique as the check_admin census; today the one hit is src/main.rs:669.
        # This counts CALL SITES in the tree. Two replicas, or an old and a new process
        # overlapping during a deploy, each spawn a drainer from that one site, and no
        # source scan can see that. Until 2026-09-22 the one site commented out still read 1
        # while zero drainers were spawned (planted); NOT_IN_A_LINE_COMMENT makes that a 0: RED.
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT + r"(?<!fn )spawn_notification_worker\("),
        "relation": "equal",
        "why": "the store takes no row claim (0 FOR UPDATE / SKIP LOCKED), so a second call "
               "site would put two drainers on the same pending rows, either free to send a "
               "row the other is sending; this row keeps 'exactly one is spawned' "
               "(notification_queue.t27:522, :687; re-pinned 2026-09-26 -- the base's :392 and "
               ":556-557 sat three and four lines above the sentence) true of the code, per "
               "process and not per deployment",
    },
    {
        # Since 2026-09-26 the only `"kind" =>` arms in the file are DeliverableKind::of's:
        # build_message matches the enum, and its friend_watered arm and raw-column catch-all
        # are gone (HELD_KINDS_DECIDED_AT). The witness pins the refusing default arm right
        # after the last kind, the shape the value depends on and the extractor cannot see:
        # a default arm that answered Some(..) would deliver every held row.
        "name": "notification_queue.RENDERABLE_KINDS ~ DeliverableKind::of arms",
        "spec": "specs/turbobaby/notification_queue.t27",
        "const": "RENDERABLE_KINDS",
        "source": "src/notification_queue.rs",
        "extract": ("regex_all", r"^\s*\"(\w+)\" =>"),
        "relation": "list_equal",
        "witness": r"\"milestone\" => Some\(Self::Milestone\),\s*_ => None,",
        "why": "a kind the drain accepts is a kind a customer can receive, and every other "
               "row is held unsent (HELD_KINDS_DECIDED_AT); an arm added here without the "
               "contract delivers a kind nobody cleared, and the retired friend_watered is the "
               "one most likely to be put back",
    },
    {
        "name": "notification_queue.WRITTEN_KINDS ~ insert_queue_row call sites",
        "spec": "specs/turbobaby/notification_queue.t27",
        "const": "WRITTEN_KINDS",
        "source": "src/db/notifications.rs",
        "extract": ("regex_all", r"insert_queue_row\(orm, \w+, \"(\w+)\""),
        "relation": "list_equal",
        "why": "the drain delivers exactly the kinds a producer writes and holds every other "
               "row (HELD_KINDS_DECIDED_AT); a producer kind missing from this list is a "
               "message held in silence, which tests/notification_drain_wiring.rs also refuses",
    },
    {
        "name": "promo_broadcast.ADMIN_API_ATTEMPTS_PER_WINDOW ~ admin.rs BROADCAST_RL_MAX_ATTEMPTS",
        "spec": "specs/turbobaby/promo_broadcast.t27",
        "const": "ADMIN_API_ATTEMPTS_PER_WINDOW",
        "source": "src/api/admin.rs",
        "extract": ("regex", r"const BROADCAST_RL_MAX_ATTEMPTS:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the only repeat guard on the route that messages EVERY customer and honours no "
               "unsubscribe; one more attempt per window is one more message to the whole base",
    },
    {
        "name": "promo_broadcast.ADMIN_API_COOLDOWN_SECONDS ~ admin.rs BROADCAST_RL_WINDOW",
        "spec": "specs/turbobaby/promo_broadcast.t27",
        "const": "ADMIN_API_COOLDOWN_SECONDS",
        "source": "src/api/admin.rs",
        "extract": ("regex", r"const BROADCAST_RL_WINDOW:[^=]*=\s*[\w:]*Duration::from_secs\(([^)]+)\)"),
        "relation": "equal",
        "why": "the other half of that guard; the contract notes a grep for cooldown misses it "
               "because it is spelled as a rate limit, so this row is how the figure stays found",
    },
    {
        "name": "promo_broadcast.SUBJECT_KINDS ~ trios/promo.rs Subject::kind",
        "spec": "specs/turbobaby/promo_broadcast.t27",
        "const": "SUBJECT_KINDS",
        "source": "src/trios/promo.rs",
        # `{ .. } => "..."` occurs only in kind(); deeplink_target() maps to Kind::, not
        # strings, and digest_icon() in the same file keys on the words as bare `"x" =>`.
        "extract": ("regex_all", r"Subject::\w+ \{ \.\. \} => \"([^\"]*)\""),
        "relation": "list_equal",
        "why": "the word travels into the dedup key, and src/trios/promo.rs:125-127 says "
               "changing one re-promotes everything of that kind; that file's unit tests pin "
               "kind() or the dedup key for set, event and event_soon, and for accessory, tea "
               "and bestseller no test does",
    },
    {
        "name": "promo_broadcast.ADMIN_ROUTE_PROMO_MUTED_MENTIONS ~ admin.rs absence",
        "spec": "specs/turbobaby/promo_broadcast.t27",
        "const": "ADMIN_ROUTE_PROMO_MUTED_MENTIONS",
        "source": "src/api/admin.rs",
        "extract": ("regex_count", r"promo_muted"),
        # Zero expected (D16): the witness is the broadcast route's own recipient query,
        # so the absence is measured in the file that holds the route. It sees the opt-out
        # only when admin.rs spells the table: a route that came to honour it through a
        # helper in another module would keep this at 0.
        "witness": r"SELECT telegram_id FROM user_languages",
        "relation": "equal",
        "why": "the contract's central finding -- the path with a cooldown ignores the "
               "unsubscribe -- rests on this zero; when the route starts reading the opt-out, "
               "path_honours_optout and path_is_fully_guarded must change with it",
    },
    {
        "name": "deeplink.PRODUCT_PREFIXES ~ deeplink.rs Kind::prefix",
        "spec": "specs/turbobaby/deeplink.t27",
        "const": "PRODUCT_PREFIXES",
        "source": "src/trios/deeplink.rs",
        # The lookahead confines the scan to the arms of prefix(): each must be followed
        # only by more arms, the close of that match and fn, any doc lines, and then
        # `pub fn wire(`. wire() and the route table use the same `Kind::X => "..."`
        # shape (15 loose hits). Keyed on the next function's NAME, not on the words of
        # its doc comment, so rewording that comment cannot blind the row. Not read here:
        # the builders' own copies, payload_prefix in src/api/share.rs and the broadcast
        # CTA in src/api/admin.rs; a rename in one of those alone breaks NEW links unseen.
        "extract": ("regex_all", r"Kind::\w+ => \"([^\"]*)\",(?=(?:\s*Kind::\w+ => \"[^\"]*\",)*\s*\}\s*\}\s*(?:///[^\n]*\n\s*)*pub fn wire\()"),
        "relation": "list_equal",
        "why": "shared links do not expire (deeplink.t27:368-370); renaming a prefix makes every "
               "card link already sent unparseable, and the customer sees only the bot's plain "
               "welcome",
    },
    {
        "name": "deeplink.PAYLOAD_MAX_LEN ~ deeplink.rs MAX_START_PARAM_LEN",
        "spec": "specs/turbobaby/deeplink.t27",
        "const": "PAYLOAD_MAX_LEN",
        "source": "src/trios/deeplink.rs",
        "extract": ("regex", r"pub const MAX_START_PARAM_LEN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "Telegram's own start-parameter cap; raising it lets the builder emit links "
               "Telegram will not carry at all, and deeplink.t27:266-268 says such a link is "
               "never delivered rather than delivered badly",
    },
    {
        "name": "deeplink.SOURCE_SEGMENT_MAX_LEN ~ deeplink.rs usable_source",
        "spec": "specs/turbobaby/deeplink.t27",
        "const": "SOURCE_SEGMENT_MAX_LEN",
        "source": "src/trios/deeplink.rs",
        # Anchored on the function signature: the loose `s.len() <= (\d+)` is also one
        # hit today, but only the signature ties the number to usable_source.
        "extract": ("regex", r"fn usable_source\(s: &str\) -> bool \{\s*!s\.is_empty\(\)\s*&& s\.len\(\) <= (\d+)"),
        "relation": "equal",
        "why": "one bound read on both sides: product_payload_from drops an over-bound "
               "segment at build, so the link ships unattributed and its post is never "
               "credited, and parse() leaves an over-bound segment glued to the product id; "
               "the invariant that the segment can never fill the payload rests on it",
    },
    {
        "name": "deeplink.PAYLOAD_LOCALE_GREP_HITS ~ deeplink.rs absence",
        "spec": "specs/turbobaby/deeplink.t27",
        "const": "PAYLOAD_LOCALE_GREP_HITS",
        "source": "src/trios/deeplink.rs",
        # Zero expected (D16): the witness proves this is still the parser's file.
        # The command the contract first cited (deeplink.t27:469, `grep -cin
        # "lang|locale"`) is a basic regex, where `|` is a literal, so it could not count a
        # line naming lang or locale alone; the contract records that correction, dated
        # 2026-09-22, at :475-481. The pattern here is a real alternation, case-folded.
        # (Re-pointed 2026-09-24: :454 and :448-452 until the rental-only correction added
        # fifteen lines above them; :448-452 had already missed the correction by twelve.)
        "extract": ("regex_count", r"(?i)lang|locale"),
        "witness": r"pub fn parse\(payload: &str\) -> Option<Target>",
        "relation": "equal",
        "why": "deeplink.t27:469-474 rests 'a forwarded link cannot pin a stranger to the "
               "sender's language' on this grep being zero; the day the grammar grows a locale "
               "segment, that paragraph is false",
    },
    # --- money-game group: loyalty ledger, referral program, webapp bridge, ride game ----------------
    # Added 2026-09-22. Each row was measured both sides by hand at a33e500 and went RED for at
    # least one drift its `why` names: a file row through --source-override on a planted copy,
    # a tree_* row by running this script from a mirror of the tracked tree with one file
    # planted in it. Not for every drift a `why` names: review the same day found green the two
    # gate counts on a gate COMMENTED OUT, GRANT_CEILING_MAJOR_UNITS on one route no longer
    # testing the constant, BALANCE_WRITE_SITE_COUNT on a raw-SQL write continued after
    # `SET \` or written in lowercase, and DEFAULTS_ON_A_BARE_NUMBER on a doc line after the
    # attribute or a private field. Each is fixed below -- a prefix, a companion row, a wider
    # pattern -- and was planted RED on exactly that drift. Two more counts of the same
    # comment-blind class, found while fixing those and not planted by the review, carry the
    # prefix too and were planted RED on a commented-out line: KEY_WRITE_SITES and
    # AUTH_HEADER_SITES.
    {
        "name": "loyalty_ledger.ADMIN_GATED_ROUTE_COUNT ~ loyalty.rs check_admin calls",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "ADMIN_GATED_ROUTE_COUNT",
        "source": "src/api/loyalty.rs",
        # :230 add_bonus, :418 use_bonus, :742 update_loyalty_config. The import at :10
        # is `check_admin,` with no paren, and the test module (:774-) calls none. A gate
        # commented out -- whole line or behind a trailing `//` -- is not counted since
        # 2026-09-22 (NOT_IN_A_LINE_COMMENT); before that it read 3 and stayed green.
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"(?<!fn )\bcheck_admin\("),
        "relation": "equal",
        "why": "the admin gate is the only thing standing on the two routes that credit "
               "and debit bonus money (loyalty_ledger.t27 MONEY_ROUTE_GATE); a handler "
               "that loses it lets any caller mint spendable checkout balance. The global "
               "GATE_CALL_SITES_ADMIN census misses a gate MOVED out of this file",
    },
    {
        "name": "loyalty_ledger.ROUTE_REGISTRATION_COUNT ~ loyalty.rs method handlers",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "ROUTE_REGISTRATION_COUNT",
        "source": "src/api/loyalty.rs",
        # The companion the row above needs: a NEW ungated route in this file leaves the admin
        # count at 3. Method HANDLERS, not `.route(` calls -- today eight, one per registration
        # at :23-33 (/loyalty/config twice, GET and POST) -- so a `.post(h)` chained onto an
        # existing registration counts too. `headers.get("..")` and `map.get(k)` do not match.
        # METHOD_HANDLER, which says what it does not see: a route commented out in place.
        "extract": ("regex_count", METHOD_HANDLER),
        "relation": "equal",
        "why": "the contract's gate distribution (3 ungated + 2 owner + 3 admin) is written "
               "against eight routes; a ninth route beside the money routes that calls no "
               "gate leaves every gate count green, and only this count moves",
    },
    {
        "name": "loyalty_ledger.GRANT_CEILING_MAJOR_UNITS ~ loyalty.rs ADD_BONUS_MAX_AMOUNT",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "GRANT_CEILING_MAJOR_UNITS",
        "source": "src/api/loyalty.rs",
        # `1_000_000.0` reads as 1000000: parse_number drops separators and folds an
        # integral float to int.
        "extract": ("regex", r"const ADD_BONUS_MAX_AMOUNT:\s*f64\s*=\s*([0-9_.]+)\s*;"),
        "relation": "equal",
        # This row reads the DECLARATION only. That both routes TEST it is the next row's
        # fact: with the debit test rewritten against an inline literal this row stayed
        # green (planted 2026-09-22).
        "why": "the one per-call ceiling the money routes test; the contract's whole admission "
               "algebra and its 'grant capped, balance not' invariant are written against "
               "this figure",
    },
    {
        "name": "loyalty_ledger.ROUTES_TESTING_THE_GRANT_CEILING ~ loyalty.rs ceiling tests",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "ROUTES_TESTING_THE_GRANT_CEILING",
        "source": "src/api/loyalty.rs",
        # Added 2026-09-22. The comparisons that test the constant, :217 (credit,
        # `req.amount > ADD_BONUS_MAX_AMOUNT`) and :405 (debit), comment lines excluded.
        # Measured 2. Its own constant, not MONEY_ROUTE_COUNT: that is a different fact which
        # happens to share the number, and the contract holds the two equal by an assert.
        # Planted RED: the debit test against an inline 100_000_000.0; the credit test deleted.
        # Not seen: a route that tests the constant through another spelling (`>=`, a helper).
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"\bamount > ADD_BONUS_MAX_AMOUNT\b"),
        "relation": "equal",
        "why": "a money route that stops testing the constant loses the per-call ceiling and "
               "nothing else notices: the declaration row above stays green, and the "
               "extra-zeros typo the ceiling exists for (loyalty.rs:193-200) reaches a balance",
    },
    {
        "name": "loyalty_ledger.IDEMPOTENCY_HEADER ~ loyalty.rs both idem_key reads",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "IDEMPOTENCY_HEADER",
        "source": "src/api/loyalty.rs",
        # The capture keeps its quotes so literal_element returns the bare string.
        "extract": ("regex", r'let idem_key: Option<String> = headers\s*\.get\(("[^"]*")\)'),
        # add_bonus (:238-242) and use_bonus (:430-434) each read it; the contract cites
        # both, so two is what this binding demands and both must spell it the same.
        "occurrences": 2,
        "relation": "equal",
        "why": "a renamed header is read as ABSENT, and an absent key is not an error "
               "here (IDEMPOTENCY_HEADER_IS_REQUIRED false) -- every client retry of a "
               "credit or debit would silently move money twice",
    },
    {
        "name": "loyalty_ledger.BALANCE_WRITE_SITE_COUNT ~ src/ balance-write census",
        "spec": "specs/turbobaby/loyalty_ledger.t27",
        "const": "BALANCE_WRITE_SITE_COUNT",
        "source": "src/**/*.rs",
        # Three shapes of write, comment lines excluded (NOT_IN_A_LINE_COMMENT, which this row
        # spelled inline first): a SeaORM col_expr on the column (9 today), raw SQL assigning
        # bonus_balance anywhere in a SET list, and an ActiveModel assignment (0). The SQL
        # alternative is case-insensitive -- SQL keywords and unquoted names are -- and lets a
        # `\` line continuation or a line break stand after SET itself as well as after a comma
        # in the list (1 today, src/bot/callbacks.rs:735). Until 2026-09-22 it read an
        # uppercase SET followed by a space only, and both `SET \`+newline and a lowercase
        # `set` were planted green; both are RED now, and the count stayed 10. Measured
        # 2026-09-22: loyalty.rs 2, db/orders.rs 1, db/referrals.rs 3 (the paired six),
        # api/orders.rs 3, bot/callbacks.rs 1 (the four bypasses). NOT seen: sea_query
        # `.value(..BonusBalance..)` (unused in src), a bare imported BonusBalance column, and
        # an ActiveModel struct literal used for an update -- the last is textually the
        # seeding insert the contract excludes.
        "extract": ("tree_regex_count",
                    NOT_IN_A_LINE_COMMENT
                    + r"(?:\.col_expr\(\s*(?:\w+::)+BonusBalance\b"
                    r'|(?i:\bSET[\s\\]+(?:[^;"]*?,[\s\\]*)?bonus_balance\s*=)'
                    r"|\.bonus_balance\s*=\s*Set\()"),
        "relation": "equal",
        "why": "the ledger-does-not-sum finding is a partition of exactly these writes "
               "into six paired and four bypasses; an eleventh balance movement is either "
               "a fifth bypass or another pairing, and nobody has classified it",
    },
    {
        "name": "referral_program.OWNER_GATED_ROUTE_COUNT ~ api/referrals.rs check_owner",
        "spec": "specs/turbobaby/referral_program.t27",
        "const": "OWNER_GATED_ROUTE_COUNT",
        "source": "src/api/referrals.rs",
        # :68, :88, :124, :164. The test module (:232-) calls none. `\bcheck_owner\(` does
        # not match check_owner_lenient(, so a downgrade to the lenient gate is a 3: RED. So,
        # since 2026-09-22, is a gate commented out (NOT_IN_A_LINE_COMMENT; green before).
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"(?<!fn )\bcheck_owner\("),
        "relation": "equal",
        "why": "four routes prove the caller IS the account whose invitee graph they "
               "read; a dropped gate publishes one person's referrals to anyone, which "
               "is the exposure referral_program.t27 already records on the fifth route",
    },
    {
        "name": "referral_program.ROUTE_COUNT ~ api/referrals.rs method handlers",
        "spec": "specs/turbobaby/referral_program.t27",
        "const": "ROUTE_COUNT",
        "source": "src/api/referrals.rs",
        # Method HANDLERS, not `.route(` calls: today five, one GET per registration at
        # :24-42. A `.post(h)` chained onto an existing registration adds a route the gate
        # census cannot see and `.route(` does not count either. The lookbehinds admit only a
        # method router standing alone or chained after `)` / at a line start; `headers
        # .get("..")` and `map.get(k)` do not match. The same pattern (METHOD_HANDLER) reads 8
        # on loyalty.rs; a route commented out in place is still counted.
        "extract": ("regex_count", METHOD_HANDLER),
        "relation": "equal",
        "why": "the gate census above cannot see a route that calls no gate; this count "
               "can, so the pair pins UNGATED_ROUTE_COUNT = 5 - 4 = 1 and a second "
               "ungated route over the referral graph -- a new path or a method chained onto "
               "an old one -- goes red instead of shipping",
    },
    {
        "name": "referral_program.MILESTONE_RUNG_COUNT ~ referrals.rs ladder length",
        "spec": "specs/turbobaby/referral_program.t27",
        "const": "MILESTONE_RUNG_COUNT",
        "source": "src/db/referrals.rs",
        # The array's declared length, not its values: MILESTONE_THRESHOLDS [1, 3, 5] is a
        # numeric array and regex_list/regex_all yield strings, so the values cannot be
        # compared by this gate today.
        "extract": ("regex", r"const MILESTONE_THRESHOLDS:\s*\[i32;\s*(\d+)\]\s*="),
        "relation": "equal",
        "why": "how many rungs the customer is shown and can reach; src/db/referrals.rs:"
               "705-711 records that this list was collapsed from three copies precisely "
               "so a rung could not be promised by one copy and paid by none",
    },
    {
        "name": "referral_program.MILESTONE_RUNG_COUNT ~ referrals.rs defaults length",
        "spec": "specs/turbobaby/referral_program.t27",
        "const": "MILESTONE_RUNG_COUNT",
        "source": "src/db/referrals.rs",
        # The second copy of the ladder the contract names (DEFAULTS_CARRY_A_SECOND_COPY_
        # OF_THE_LADDER). Bound to the SAME constant as the row above on purpose.
        "extract": ("regex", r"const MILESTONE_DEFAULT_BONUS:\s*\[\(i32,\s*f64\);\s*(\d+)\]\s*="),
        "relation": "equal",
        "why": "a rung added to MILESTONE_THRESHOLDS alone gets no default; unless "
               "loyalty_config also names milestone_bonus_N it resolves to zero, the award "
               "loop skips it, and it is still published to the customer "
               "(a_new_rung_can_be_offered_and_never_paid); with both rows bound, updating "
               "the contract for the ladder turns this row red until the defaults follow",
    },
    {
        "name": "webapp_bridge.KEY_WRITE_SITES ~ checkout_screen.rs key signal writes",
        "spec": "specs/turbobaby/webapp_bridge.t27",
        "const": "KEY_WRITE_SITES",
        "source": "src/ui/screens/checkout_screen.rs",
        # :970 `idempotency_key.write()` feeding get_or_insert_with at :971. The verbs are
        # every mutating method dioxus-signals 0.6.3 (Cargo.lock) gives a
        # Signal<Option<String>> -- Writable plus WritableOptionExt, src/write.rs. `take()`
        # and `replace(..)` are the idiomatic reset and re-mint, and a write/set-only list
        # read both as nothing. The &str param of the same name in submit_order_with_retry
        # (:151) has none of these called on it. Comment lines excluded since 2026-09-22
        # (NOT_IN_A_LINE_COMMENT): the one write commented out now reads 0: RED.
        "extract": ("regex_count",
                    NOT_IN_A_LINE_COMMENT
                    + r"\bidempotency_key\.(?:write|try_write|write_unchecked|try_write_unchecked"
                    r"|with_mut|set|take|replace|get_or_insert|get_or_insert_with|as_mut"
                    r"|map_mut|try_map_mut)\("),
        "relation": "equal",
        "why": "one write is why a retry cannot become a second order; a second write "
               "(a reset after failure, a re-mint on edit) hands the retry loop a fresh "
               "key and POST /api/orders can create the order twice",
    },
    {
        "name": "webapp_bridge.DEFAULTS_ON_A_BARE_NUMBER ~ types.rs serde default on a number",
        "spec": "specs/turbobaby/webapp_bridge.t27",
        "const": "DEFAULTS_ON_A_BARE_NUMBER",
        "source": "src/ui/api/types.rs",
        # Attribute POSITION, not text: the comment at :37 and the doc line at :426 name the
        # attribute without being one. Any serde attribute whose arguments include
        # `default` counts -- this file already writes it combined twice
        # (:121 `rename = "type", default`, :130), on a String and a bool today -- and other
        # attributes, and since 2026-09-22 `//` and `///` lines, may sit between it and the
        # field, which may be private. Until then a doc line after the attribute, or a field
        # without `pub`, hid a seventh default (both planted green; both RED now). Today :28,
        # :40, :311, :423, :450, :461, the six NUMERIC_DEFAULT_SITES, re-measured 6 with the
        # wider gap (:421, :448, :459 and the doc line at :424 until 2026-09-24, when the two
        # DeliveryZone ETA fields became Options with a default -- not numbers, so still 6). Re-checked against webapp_bridge.t27's 2026-09-22 correction: the constant
        # counts the bare attribute, and the contract records COMBINED_DEFAULTS_ON_A_NUMBER =
        # 0, so counted by what serde does the numeric column is the same six. This row reads
        # the serde reading -- any spelling of `default` -- so a combined default on a number
        # is RED here although it would leave the bare count at six: the zero-fabricating
        # column is what the `why` is about. Not seen: a struct-level #[serde(default)] (the
        # contract records none in the file), and a default on a number in one of the two
        # Serialize-only structs is counted although serde never parses it -- a false red.
        "extract": ("regex_count",
                    r"^[ \t]*#\[serde\([^\]\n]*\bdefault\b[^\]\n]*\)\]\s*"
                    r"(?:#\[[^\]\n]*\]\s*|//[^\n]*\n\s*)*"
                    r"(?:pub(?:\([^)]*\))?\s+)?\w+: "
                    r"(?:f32|f64|i8|i16|i32|i64|i128|isize|u8|u16|u32|u64|u128|usize),"),
        "relation": "equal",
        "why": "each of these turns a field the server stopped sending into a zero it "
               "never sent; ServerCart.total and both quantities are among them, and "
               "unit_price leaving this list is the fix webapp_bridge.t27 records -- a "
               "seventh is a price or count the client can fabricate again",
    },
    {
        "name": "webapp_bridge.AUTH_HEADER_SITES ~ src/ui/api census",
        "spec": "specs/turbobaby/webapp_bridge.t27",
        "const": "AUTH_HEADER_SITES",
        "source": "src/ui/api/*.rs",
        # http.rs 8, client.rs 1. Case-insensitive because header names are: 44 other client
        # sites (admin_screen.rs 34 and five more files, all outside this glob and outside
        # the contract's 'two transport modules') spell it "X-Telegram-Init-Data", so the
        # likeliest new helper is a paste in that spelling. The label header
        # "x-telegram-init-data-reconstructed" does not match: the closing quote is in the
        # pattern. Comment lines excluded since 2026-09-22 (NOT_IN_A_LINE_COMMENT): a send
        # commented out is a helper that stopped authenticating, and read 9 before.
        "extract": ("tree_regex_count", r"(?i)" + NOT_IN_A_LINE_COMMENT + r'header\(\s*"x-telegram-init-data"'),
        "relation": "equal",
        "why": "the census the label finding is a fraction of (seven of nine send it); a "
               "new authed helper in the transport modules moves this and forces the "
               "decision whether it tells the server a rebuilt payload is rebuilt, which "
               "the missing label turns into the signed-but-invalid cohort",
    },
    {
        "name": "webapp_bridge.BACKOFF_STEP_COUNT ~ checkout_screen.rs DELAYS_MS",
        "spec": "specs/turbobaby/webapp_bridge.t27",
        "const": "BACKOFF_STEP_COUNT",
        "source": "src/ui/screens/checkout_screen.rs",
        # Anchored on the one retrying function. BACKOFF_MS itself ([1000, 2000, 4000]) is a
        # numeric array and cannot be compared by this gate today; ATTEMPT_MAX (4) is not a
        # literal anywhere -- it is once(0).chain(DELAYS_MS), i.e. this number plus one.
        "extract": ("regex", r"fn submit_order_with_retry\([^)]*\)[^{]*\{\s*const DELAYS_MS:\s*\[u32;\s*(\d+)\]"),
        "relation": "equal",
        "why": "the length of this array is the retry count on POST /api/orders (ATTEMPT_MAX "
               "= this + 1); the doc comment misstated it once already, and the contract is "
               "where the worst case per tap is sized",
    },
    {
        "name": "ride_game.MIN_UNITS_AVAILABLE_TO_RIDE ~ ride.js unit floor",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "MIN_UNITS_AVAILABLE_TO_RIDE",
        "source": "assets/game/ride.js",
        # ride.js:243 in rideableFamilies.
        "extract": ("regex", r"if \(!Number\.isInteger\(units\) \|\| units < (\d+)\) continue;"),
        "relation": "equal",
        "why": "the in-page floor rideableFamilies applies before mounting a family. It is "
               "the SECOND guard: available_only on the roster URL already drops families "
               "with no free unit server-side, and ride.js:229-232 keeps this check for a "
               "caller that points rosterUrl elsewhere. At 0 that caller -- or everyone, "
               "the day RIDEABLE_SOURCE loses its filter -- is offered a machine the shop "
               "cannot hand over (D9/D12)",
    },
    {
        "name": "ride_game.RIDEABLE_SOURCE ~ ride.js ROSTER_URL",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "RIDEABLE_SOURCE",
        "source": "assets/game/ride.js",
        # ROSTER_URL is only the DEFAULT: ride.js:323 reads `opts.rosterUrl || ROSTER_URL`.
        # Measured 2026-09-22, no host passes rosterUrl (src/ui/screens/ride_screen.rs hands
        # the module strings and callbacks only). The day one does, this row pins a dead
        # default and stays green.
        "extract": ("regex", r"^const ROSTER_URL = ('[^']*');"),
        "relation": "equal",
        "why": "the rideable set is read at mount time from this query and from no list "
               "held in the game; losing available_only hands the client every family "
               "the catalog publishes and leaves the in-page filter as the only guard",
    },
    {
        "name": "ride_game.CATALOG_KEYS ~ 082 family rows",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "CATALOG_KEYS",
        "source": "migrations/082_bikes_seed.sql",
        "extract": ("regex_all", FAMILY_ROW_SQL),
        "relation": "list_equal",
        "why": "ride_game.t27 declares CATALOG_KEYS a COPY of availability.FAMILY_KEYS, and "
               "its own invariants hold the copy only to the owner's length and two end keys, "
               "written as literals in the same file; binding the copy to the same SQL its "
               "owner is bound to makes a one-sided edit of any of the fourteen red, and the "
               "subset invariant over RIDEABLE_KEYS is only as good as this list",
    },
    {
        "name": "ride_game.CLASS_NAMES ~ bikes.rs BIKE_CLASSES",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "CLASS_NAMES",
        "source": "src/api/bikes.rs",
        "extract": ("regex_list", r"const BIKE_CLASSES:\s*\[&str;\s*\d+\]\s*=\s*\[(.*?)\]\s*;"),
        "relation": "list_equal",
        "why": "steer_rate and ride.js STEER_RATE_SU are a CLOSED two-class lookup; a class "
               "added to the catalog makes handlingFor return null and its bikes vanish "
               "from the game, and this copy of bike_catalog.CLASSES is where that has to "
               "be confronted -- the owner's own binding does not reach the copy",
    },
    # --- ride handling: the contract's CHOSEN game-unit map against the shipped module ---
    # Added 2026-09-24. ride_game.t27 declares the handling map (top_speed, steer_rate) and says
    # the five numbers in it are CHOSEN and measured nowhere; assets/game/ride.js:104-110 says the
    # canonical game-unit map IS that contract. The asset was already pinned before this group:
    # tests/ride_asset_wiring.rs, test ride_handling_and_silhouettes_keep_separate_catalog_inputs,
    # holds the literals 400, 1 and { scooter: 120, motorcycle: 90 } and both consuming lines under
    # cargo test, so an edit to ride.js alone was already red. What nothing tied was the asset to
    # the CONTRACT: an edit to ride_game.t27 alone, or an edit to ride.js made together with the
    # matching edit to that test's literals, passed every gate. These rows close that. Each row
    # reads one declaration line at column zero, so a second declaration of the same name, a key
    # order swapped inside STEER_RATE_SU, or the constant moved into a block turns it RED rather
    # than green. The witness pins the one expression in handlingFor that consumes the constants
    # (ride.js:137-138), the same lines the Rust test pins: a value that still matches while the
    # formula stops using it -- a multiply turned into an add, a lookup replaced by a literal --
    # is the drift the value alone cannot see. Planted RED 2026-09-24 through --source-override
    # on a scratch copy of ride.js with 400 -> 401, 1 -> 2, scooter 120 -> 121 and motorcycle
    # 90 -> 91, one at a time, and with the ride.js:137 expression rewritten as
    # INTERCEPT * PER_CC + cc (both speed witnesses).
    {
        "name": "ride_game.SPEED_INTERCEPT_GU ~ ride.js SPEED_INTERCEPT_GU",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "SPEED_INTERCEPT_GU",
        "source": "assets/game/ride.js",
        "extract": ("regex", r"^const SPEED_INTERCEPT_GU = (\d+);"),
        "witness": r"const topSpeedGu = SPEED_INTERCEPT_GU \+ SPEED_PER_CC_GU \* cc;",
        "relation": "equal",
        "why": "the intercept is the CHOSEN compression of the fleet's 6.0x displacement span "
               "to 2.19x in top speed; ride.js evaluates it before mounting a rider, so a "
               "local re-tune -- even one the Rust asset test's literal is moved to match -- "
               "moves every family's handling away from the contract's published game-unit "
               "map",
    },
    {
        "name": "ride_game.SPEED_PER_CC_GU ~ ride.js SPEED_PER_CC_GU",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "SPEED_PER_CC_GU",
        "source": "assets/game/ride.js",
        "extract": ("regex", r"^const SPEED_PER_CC_GU = (\d+);"),
        "witness": r"const topSpeedGu = SPEED_INTERCEPT_GU \+ SPEED_PER_CC_GU \* cc;",
        "relation": "equal",
        "why": "the slope is what makes top speed strictly increasing in displacement (the "
               "contract's invariant is SPEED_PER_CC_GU >= 1); at 0 every family rides "
               "identically and the whole displacement span is decorative (issue #13)",
    },
    {
        "name": "ride_game.STEER_RATE_SCOOTER_SU ~ ride.js STEER_RATE_SU.scooter",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "STEER_RATE_SCOOTER_SU",
        "source": "assets/game/ride.js",
        "extract": ("regex", r"^const STEER_RATE_SU = \{ scooter: (\d+), motorcycle: \d+ \};"),
        "witness": r"const steerRateSu = STEER_RATE_SU\[klass\];",
        "relation": "equal",
        "why": "the scooter half of the closed two-class steering lookup; the contract grounds "
               "only its DIRECTION (scooters steer quicker, KB_faq's beginner advice), so a "
               "value moved in ride.js alone is a new design choice made outside the contract",
    },
    {
        "name": "ride_game.STEER_RATE_MOTORCYCLE_SU ~ ride.js STEER_RATE_SU.motorcycle",
        "spec": "specs/turbobaby/ride_game.t27",
        "const": "STEER_RATE_MOTORCYCLE_SU",
        "source": "assets/game/ride.js",
        "extract": ("regex", r"^const STEER_RATE_SU = \{ scooter: \d+, motorcycle: (\d+) \};"),
        "witness": r"const steerRateSu = STEER_RATE_SU\[klass\];",
        "relation": "equal",
        "why": "the motorcycle half of the same lookup; the chosen 4:3 ratio against the "
               "scooter rate is the only thing that separates the classes in the game, and "
               "the contract's steer_rate_separates_the_two_classes test reads these numbers",
    },
    # --- tree group: schema provenance, runtime config, legacy retirement, observability, publication ---
    # Added 2026-09-22. Each row was measured both sides by hand at a33e500 and went RED for at
    # least one drift its `why` names: a file row through --source-override on a planted copy, a
    # tree_* row by running this script from a mirror of the tracked tree with one file planted
    # in it. Not for every drift a `why` names: review the same day found green
    # FAIL_OPEN_CALL_SITES on a site moving between files at an unchanged total, the bot-handle
    # census on a handle in another letter case, SPAWN_EXPRESSIONS_IN_SHIPPED_CODE on a spawn
    # commented out, the three legacy route counts on a method chained onto an existing
    # registration, and REQUIRED_GENERATOR_COUNT on the gate's loop narrowed to a slice. Each is
    # fixed below -- a companion row, a flag, a prefix, a different count, a witness -- and was
    # planted RED on exactly that drift. Three more counts of the comment-blind class, found
    # while fixing those and not planted by the review, carry the prefix too and were planted
    # RED on a commented-out line: FAIL_OPEN_CALL_SITES, WEB_APP_URL_DENYLIST_ENTRIES and
    # EMITTED_METRIC_NAME_COUNT. REQUIRED_ALWAYS stays comment-blind: its two names share one
    # line, which the prefix would count once.
    {
        "name": "schema_provenance.BACKFILL_BOUNDARY ~ mod.rs LAST_PRE_BIKE_MIGRATION",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "BACKFILL_BOUNDARY",
        "source": "src/db/mod.rs",
        "extract": ("regex", r'const LAST_PRE_BIKE_MIGRATION:\s*&str\s*=\s*("[^"]*")\s*;'),
        "relation": "equal",
        "why": "the one-time backfill marks every migration up to this name applied WITHOUT "
               "running it; moved forward to a bike migration, an established database never "
               "creates the bike tables and the catalog 500s on a schema that looks migrated",
    },
    {
        "name": "schema_provenance.ESTABLISHED_PROBE_TABLE ~ mod.rs to_regclass probe",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "ESTABLISHED_PROBE_TABLE",
        "source": "src/db/mod.rs",
        "extract": ("regex", r"to_regclass\(('[^']*')\)\s*IS NOT NULL AS est"),
        "relation": "equal",
        "why": "D3's probe routes a boot between backfill and apply-everything; keyed on a "
               "table a later migration drops (public.strains, dropped by 083) it answers "
               "'fresh' for a live database and re-applies 001-076, seeds included, over it",
    },
    {
        "name": "schema_provenance.FAIL_OPEN_SITES_ON_BIKE_PATH ~ bike modules absence",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "FAIL_OPEN_SITES_ON_BIKE_PATH",
        "source": "src/**/*bike*.rs",
        "extract": ("tree_regex_count", r"try_get_warn!\s*[\(\[\{]"),
        # Expected value is zero (D16). The witness is the macro's NAME in the doc
        # comments of src/api/bikes.rs, src/db/bikes.rs and src/db/entities/bike.rs that
        # record the decision to keep it off this path; a rename of the macro or a
        # deletion of those sentences turns this red instead of leaving a blind zero.
        "witness": r"try_get_warn!",
        "relation": "equal",
        "why": "a try_get_warn! read of base_rate_thb_day or deposit_thb returns the typed "
               "default on a renamed column, and for a rate that default is 0, which the "
               "customer reads as FREE; the contract declares the bike path's zero a decision",
    },
    {
        "name": "schema_provenance.FAIL_OPEN_CALL_SITES ~ src/ try_get_warn census",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "FAIL_OPEN_CALL_SITES",
        "source": "src/**/*.rs",
        # 24 raw hits; the contract subtracts the macro module's three references to
        # itself. The two lookarounds exclude exactly those three BY THEIR TEXT: the
        # doc example at src/db/macros.rs:16 and the two self-tests reading column "x".
        # Editing any of those three lines is a false red, the fail-closed direction.
        # Landing this row made FAIL_OPEN_GATES = 0 and FAIL_OPEN_GATE_NOTE in the contract
        # stale; the contract records the correction beside them, dated 2026-09-22. Since the
        # same day a call on a commented-out line is not a site (NOT_IN_A_LINE_COMMENT; the
        # doc example is a `///` line and is now excluded twice). A TOTAL: a site that leaves
        # one file while another appears elsewhere keeps it at 21 -- planted green -- which
        # is the next row's to see.
        "extract": ("tree_regex_count",
                    NOT_IN_A_LINE_COMMENT + r'(?<!/// let v: f64 = )try_get_warn!\s*[\(\[\{](?!r, "x", 42\))'),
        "relation": "equal",
        "why": "D9's rule is 'grep every call site before renaming a column it reads', and "
               "this row is that grep run on every build: a 22nd fail-open read anywhere "
               "under src/ moves this number, and so does one removed without the contract "
               "being re-measured; a read that MOVES is the next row's",
    },
    {
        "name": "schema_provenance.FAIL_OPEN_CALL_SITE_FILES ~ src/ try_get_warn files",
        "spec": "specs/turbobaby/schema_provenance.t27",
        "const": "FAIL_OPEN_CALL_SITE_FILES",
        "source": "src/**/*.rs",
        # Added 2026-09-22: the same pattern as the row above, counted per FILE -- measured 3,
        # src/api/catalog.rs 16, src/db/orders.rs 3, src/db/referrals.rs 2, the three
        # FAIL_OPEN_FILES. Planted RED: a site on the order-pricing read in src/api/orders.rs
        # with one taken out of catalog.rs, total 21, four files. Not seen: a site moving
        # between the three files already in the list (FAIL_OPEN_SITES_PER_FILE is a numeric
        # array, which this gate cannot compare today), nor a file swapped for another.
        "extract": ("tree_module_count",
                    NOT_IN_A_LINE_COMMENT + r'(?<!/// let v: f64 = )try_get_warn!\s*[\(\[\{](?!r, "x", 42\))'),
        "relation": "equal",
        "why": "the census is only a map of the fail-open reads while it names their files; a "
               "read that arrives in a fourth file at an unchanged total -- the price-authority "
               "lookup in src/api/orders.rs reads a price through the strict try_get today, "
               "on purpose (its Cycle #98 comment) -- is exactly the move D9 asks a human to "
               "look at",
    },
    {
        "name": "runtime_config.WEB_APP_URL_DENYLIST_ENTRIES ~ config.rs legacy-host guard",
        "spec": "specs/turbobaby/runtime_config.t27",
        "const": "WEB_APP_URL_DENYLIST_ENTRIES",
        "source": "src/config.rs",
        # Comment lines excluded since 2026-09-22 (NOT_IN_A_LINE_COMMENT): an entry commented
        # out in a tidy-up -- the drift the `why` names -- read 2 before and reads 1 now: RED.
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"\|\|\s*raw_web_app_url\.contains\("),
        "relation": "equal",
        "why": "D19: each entry is a remembered wrong host the Telegram menu button must not "
               "open; one dropped in a tidy-up re-admits the other shop's miniapp for every "
               "customer. Counted without transcribing the hostnames, as the contract requires",
    },
    {
        "name": "runtime_config.BOT_HANDLE_CLIENT_LITERAL_FILES ~ src/ui/ handle census",
        "spec": "specs/turbobaby/runtime_config.t27",
        "const": "BOT_HANDLE_CLIENT_LITERAL_FILES",
        "source": "src/ui/**/*.rs",
        # Handle-agnostic on purpose: it counts files compiling in ANY *_bot handle, so a
        # file that compiles in another business's bot is counted too, and the table does
        # not become a second copy of the value. The leading class admits the four ways a
        # client string spells a handle: a bare literal ("), a link path (t.me/...), a
        # mention (@...) and a tg:// resolve query (domain=...). Case-insensitive since
        # 2026-09-22, as Telegram usernames are: t.me/TurboAgent_Phuket_Bot in a fifth file
        # was planted green before and is RED now, and the count on today's tree is still 4.
        # Blind to a handle without `_bot` (Telegram requires only the suffix `bot`); a field
        # literal like "is_bot" would be counted, which is a fail-closed red.
        "extract": ("tree_module_count", r'(?i)["/@=][A-Za-z0-9_]*_bot\b'),
        "relation": "equal",
        "why": "the client reads BOT_USERNAME at zero sites and compiles the handle in; a "
               "fifth file doing so is a share or support link the server knob cannot reach, "
               "which is the half of D19 that is still open",
    },
    {
        "name": "runtime_config.SPAWN_EXPRESSIONS_IN_SHIPPED_CODE ~ src/ tokio::spawn census",
        "spec": "specs/turbobaby/runtime_config.t27",
        "const": "SPAWN_EXPRESSIONS_IN_SHIPPED_CODE",
        "source": "src/**/*.rs",
        # No tokio::spawn( sits under #[cfg(test)] today, so "shipped" and "all" agree;
        # a future test-module spawn is a fail-closed false red. A spawn on a commented-out
        # line is not counted since 2026-09-22 (NOT_IN_A_LINE_COMMENT; planted green before).
        # Two spawns on ONE line count once under that prefix; none shares a line today.
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT + r"tokio::spawn\("),
        "relation": "equal",
        "why": "counts spawn EXPRESSIONS, not loops: it goes red when a tokio::spawn is "
               "added or removed without re-taking the inventory, but a fourth TTL loop added "
               "through spawn_ttl_sweep (one expression, three callers) adds nothing here, and "
               "no interval change is visible to it",
    },
    {
        "name": "runtime_config.REQUIRED_ALWAYS ~ config.rs collect_required_env",
        "spec": "specs/turbobaby/runtime_config.t27",
        "const": "REQUIRED_ALWAYS",
        "source": "src/config.rs",
        # The lookahead ties the names to the array whose closure reads the real
        # environment; the six tests at :360-397 pass the same array with a `reader`
        # closure and are not counted.
        "extract": ("regex_count", r'"[A-Z][A-Z0-9_]*"(?=[^\]]*\],\s*\|name\|\s*\{?\s*std::env::var\(name\))'),
        "relation": "equal",
        "why": "the names whose absence refuses the boot in every deployment class, "
               "collected so one boot reports all of them; a name added to or dropped from "
               "that array changes which deployments can start. A variable made mandatory "
               "by some other read is outside this array and outside this row",
    },
    {
        "name": "legacy_retirement.CATALOG_HTTP_ROUTE_COUNT ~ catalog.rs route census",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "CATALOG_HTTP_ROUTE_COUNT",
        "source": "src/api/catalog.rs",
        # Method HANDLERS (METHOD_HANDLER), since 2026-09-22: until then this counted `.route("`
        # registrations, and `get(get_accessory).patch(update_accessory)` added a route at an
        # unchanged registration count -- planted green, RED now. Today every registration
        # carries one method, so both counts read 27. The contract's note that a per-line grep
        # for the `.route(` shape returns 23 (four registrations span two lines) is about
        # that grep and still holds; re-measure through this script, not a line grep. Not
        # seen: a route commented out in place (see METHOD_HANDLER).
        "extract": ("regex_count", METHOD_HANDLER),
        "relation": "equal",
        "why": "the legacy catalog router is still merged into the public API over rows 085 "
               "only hid; a route added here re-exposes another shop's catalogue, a route "
               "removed is retirement the contract must record (issue #2)",
    },
    {
        "name": "legacy_retirement.QUEST_HTTP_ROUTE_COUNT ~ quest.rs route census",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "QUEST_HTTP_ROUTE_COUNT",
        "source": "src/api/quest.rs",
        # :22-35. The /api/test registration 2c21df1 removed is recorded in a comment at :15-20
        # that deliberately spells no route shape, so no method handler in it matches either.
        # Method handlers since 2026-09-22 (METHOD_HANDLER): `.delete(h)` chained onto
        # `put(update_quest_location)` was planted green on the `.route(` count and is RED
        # now; both read 12 today.
        "extract": ("regex_count", METHOD_HANDLER),
        "relation": "equal",
        "why": "one of the three parts the contract's executed sum assert adds up to "
               "LEGACY_HTTP_ROUTE_COUNT; it went on reading 13 after 2c21df1 removed /api/test "
               "because nothing compared it with the router, and a route added or removed here "
               "is a change to the legacy surface the contract must record (issue #2)",
    },
    {
        "name": "legacy_retirement.TECH_TREE_HTTP_ROUTE_COUNT ~ tech_tree.rs route census",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "TECH_TREE_HTTP_ROUTE_COUNT",
        "source": "src/api/tech_tree.rs",
        # :14-17. With the two rows above, all three parts of the executed sum are bound; the
        # total itself spans three files, which one glob cannot name, so it is held by the sum.
        # Method handlers since 2026-09-22 (METHOD_HANDLER): a chained `.delete(h)` was planted
        # green on the `.route(` count and is RED now; both read 4 today.
        "extract": ("regex_count", METHOD_HANDLER),
        "relation": "equal",
        "why": "the third part of LEGACY_HTTP_ROUTE_COUNT; the tech-tree router is merged into "
               "the public API (src/api/mod.rs:97) and /tech-tree is one of the eight client "
               "paths the contract calls live, so a route added or removed here moves the "
               "legacy total the contract publishes",
    },
    {
        "name": "legacy_retirement.ALLOWLIST_SIZE ~ retired_table_wiring.rs SURVIVORS",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "ALLOWLIST_SIZE",
        "source": "tests/retired_table_wiring.rs",
        # Column-four indentation is what separates the SURVIVORS entries from any
        # Survivor literal a test body might build.
        "extract": ("regex_count", r'^    Survivor \{\s*path:'),
        "relation": "equal",
        "why": "each entry excuses shipped code that still reads a table 083 dropped (the "
               "cart line of kind strain 500s); the contract owns this number, bot_surface "
               "copies it, and it is the one most likely to move",
    },
    {
        "name": "legacy_retirement.GARDEN_ROUTE_DECLARATIONS ~ src/ absence",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "GARDEN_ROUTE_DECLARATIONS",
        "source": "src/**/*.rs",
        # The contract's search is over src/ (legacy_retirement.t27, the paragraph above
        # GARDEN_ROUTE_DECLARATIONS), not one file, so both declaration forms are counted: a
        # client #[route("...")] attribute and an axum .route("..."). A re-added /api/garden/*
        # router is as visible as a re-added /garden screen; comments naming the removed route
        # do not match. Zero needs a witness (D16): the same two shapes with any path.
        # Unanchored on purpose: tree witnesses are searched with no flags, so `^` would mean
        # start-of-file.
        "extract": ("tree_regex_count", r'(?:#\[route\(|\.route\()\s*"[^"]*garden'),
        "witness": r'(?:#\[route\(|\.route\()\s*"/',
        "relation": "equal",
        "why": "the one completed retirement; the word garden still appears on four lines "
               "of the client router as comments, which is why the contract counts "
               "DECLARATIONS, and a route re-added over the tables 083 dropped -- client "
               "or HTTP -- must turn this red",
    },
    {
        "name": "legacy_retirement.LEGACY_PATHS_DECLARED ~ routes.rs legacy paths",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "LEGACY_PATHS_DECLARED",
        "source": "src/ui/routes.rs",
        # The pattern spells the eight paths, so this is a presence-and-order check on
        # the route ATTRIBUTES. Which screen a path mounts is decided in its handler far
        # below, and a one-capture pattern cannot tie the two: a path kept and repointed
        # at CatalogScreen, as /sets and /sommelier are, stays green here. Until 2026-09-25
        # this row read LEGACY_LIVE_ROUTES, which held the same eight; on the owner's answer
        # of that day seven of them render the catalog (the REPOINTED row counts them), and
        # LEGACY_LIVE_ROUTES now names the one that still mounts a legacy screen.
        "extract": ("regex_all", r'^\s*#\[route\("(/accessories|/tea|/quest/:id|/game|/treasure-hunt|/ar-hunt|/location-quest|/tech-tree)"\)\]'),
        "relation": "list_equal",
        "why": "the eight legacy client paths must still be declared, in this order, because "
               "a link already sent never expires; deleting or renaming one (the /garden mode) "
               "turns this red. It does NOT see which surface a path lands on -- the REPOINTED "
               "row does -- nor a NEW legacy path",
    },
    {
        "name": "observability.REQUEST_ID_HEADER ~ observability.rs REQUEST_ID_HEADER",
        "spec": "specs/turbobaby/observability.t27",
        "const": "REQUEST_ID_HEADER",
        "source": "src/api/observability.rs",
        "extract": ("regex", r'const REQUEST_ID_HEADER:\s*HeaderName\s*=\s*HeaderName::from_static\(("[^"]*")\)\s*;'),
        "relation": "equal",
        "why": "the header a proxy or caller correlates by, read inbound and echoed outbound "
               "by the same constant; renamed, every inbound id is silently replaced by a mint",
    },
    {
        "name": "observability.REQUEST_ID_MAX_BYTES ~ observability.rs MAX_INBOUND_REQUEST_ID_LEN",
        "spec": "specs/turbobaby/observability.t27",
        "const": "REQUEST_ID_MAX_BYTES",
        "source": "src/api/observability.rs",
        "extract": ("regex", r"const MAX_INBOUND_REQUEST_ID_LEN:\s*usize\s*=\s*([0-9_]+)\s*;"),
        "relation": "equal",
        "why": "the only bound on client-supplied text that is copied into every log line and "
               "echoed on every response; the contract's refusal algebra is written against it",
    },
    {
        "name": "observability.SPAN_FIELD_NAMES ~ observability.rs http_request span fields",
        "spec": "specs/turbobaby/observability.t27",
        "const": "SPAN_FIELD_NAMES",
        "source": "src/api/observability.rs",
        "extract": ("regex_all", r'^\s+(\w+) = %\w+,\s*$(?=(?:\s+\w+ = %\w+,\s*$)*\s*\);)'),
        "relation": "list_equal",
        "why": "the part of a log line the code fixes in BOTH renderings; a log query keyed "
               "on request_id finds nothing after a rename, and nothing else notices",
    },
    {
        "name": "observability.GUARDED_DASHBOARD_PATH ~ main.rs dashboard name guard",
        "spec": "specs/turbobaby/observability.t27",
        "const": "GUARDED_DASHBOARD_PATH",
        "source": "src/main.rs",
        "extract": ("regex", r'fn dashboard_exprs_reference_real_metrics\(\)\s*\{[^}]*?\.join\(("[^"]*")\)'),
        "relation": "equal",
        "why": "the contract's finding is that the name guard reads ONE of four PromQL "
               "artifacts; repointed, the guarded set changes and every coverage number "
               "in the contract describes a different file",
    },
    {
        "name": "observability.EMITTED_METRIC_NAME_COUNT ~ metrics.rs counter!/gauge! literals",
        "spec": "specs/turbobaby/observability.t27",
        "const": "EMITTED_METRIC_NAME_COUNT",
        "source": "src/metrics.rs",
        # Counts name LITERALS, which equals distinct names today (41 and 41), all above the
        # #[cfg(test)] at :330. `\s*` crosses the newline, so the twelve `counter!(\n "name"`
        # forms count -- the form src/main.rs declared_metrics, which searches for the markers
        # `counter!("` and `gauge!("`, cannot see, and the reason the contract said 29 until
        # 2026-09-22. Comment lines excluded since the same day (NOT_IN_A_LINE_COMMENT): a
        # metric commented out is not emitted, and read 41 before; it reads 40 now: RED.
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r'\b(?:counter|gauge|histogram)!\(\s*"[a-z][a-z0-9_]*"'),
        "relation": "equal",
        "why": "the emitted set is what a dashboard or alert reference must resolve against; "
               "a metric added in the multi-line form is emitted by the binary and invisible "
               "to the dashboard name guard, and only this count moves. It counts literals, "
               "not distinct names: a second literal re-using a name moves it too",
    },
    {
        "name": "publication.REQUIRED_GENERATOR_COUNT ~ verify_t27_specs.py GENERATORS",
        "spec": "specs/turbobaby/publication.t27",
        "const": "REQUIRED_GENERATOR_COUNT",
        "source": "scripts/verify_t27_specs.py",
        # Counts only strings in a comma-run of gen* literals that closes its line with `)`,
        # i.e. the tuple. Spread over several lines with a trailing comma it reads 0: a noisy
        # red, but a red. The tuple is what is DECLARED; what the gate RUNS is its loop, and
        # `for generator in GENERATORS[:4]:` kept this count at 5 (planted 2026-09-22). The
        # witness pins the unsliced loop header; the same plant is RED now. Not seen: a
        # `continue` or filter inside the loop body.
        "extract": ("regex_count", r'"gen[a-z-]*"(?=(?:,\s*"gen[a-z-]*")*\)\s*$)'),
        "witness": r"for generator in GENERATORS:",
        "relation": "equal",
        "why": "compiler evidence is defined as non-empty output from every generator the "
               "spec gate iterates; a lane dropped from that tuple is evidence the contract "
               "still claims",
    },
    {
        "name": "publication.AGENT_CARD_ID ~ agent card ID",
        "spec": "specs/turbobaby/publication.t27",
        "const": "AGENT_CARD_ID",
        "source": "specs/agents/turbobaby.t27",
        "extract": ("regex", r'^pub const ID\s*:\s*str\s*=\s*("[^"]*")\s*;'),
        "relation": "equal",
        "why": "the namespaced ID every agent-publication verdict in the contract is about; "
               "renamed on the card, those verdicts describe an ID nothing declares",
    },
    {
        "name": "publication.AGENT_CARD_PATH ~ verify_t27_specs.py AGENT_CARD",
        "spec": "specs/turbobaby/publication.t27",
        "const": "AGENT_CARD_PATH",
        "source": "scripts/verify_t27_specs.py",
        "extract": ("regex", r'^AGENT_CARD\s*=\s*("[^"]*")\s*$'),
        "relation": "equal",
        "why": "the path the spec gate adds to discovery outside specs/turbobaby; if it "
               "moves, the contract's 'tracked source is necessary' leg points at a file "
               "no gate reads",
    },
    # --- rental only, Phuket only: the owner's ruling of 2026-09-24 -------------------------------
    # Added 2026-09-24 with the ruling that events and bike sales leave every customer surface.
    # Each row was measured on both sides by hand, on the tree before the change (f49c372) and
    # after it, and every row whose value the change moved went from one reading to the other:
    # REPOINTED 2 -> 5, mounts 3 -> 0, sale-key lines 3 -> 0, withheld reads 0 -> 2. The rest pin
    # a shape the change created (the refusal arms, the publish gate) or kept (the paths).
    {
        "name": "legacy_retirement.REPOINTED_ROUTE_COUNT ~ routes.rs handlers that are only the catalog",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "REPOINTED_ROUTE_COUNT",
        "source": "src/ui/routes.rs",
        # A handler whose WHOLE body renders the catalog: /sets and /sommelier since their
        # screens were deleted, the three events paths since 2026-09-24, and seven of the eight
        # legacy paths since the owner's answer of 2026-09-25 (5 -> 12). Home renders the
        # catalog too, but behind a branch, and Menu mounts MenuScreen, so neither matches. An
        # EventDetail keeps its `id` prop (`let _ = id;`) so an old /events/<id> still parses.
        "extract": ("regex_count", r"fn \w+\([^)]*\) -> Element \{\s*(?:let _ = \w+;\s*)?rsx! \{\s*CatalogScreen \{\}\s*\}\s*\}"),
        "relation": "equal",
        "why": "the count LEGACY_PATHS_DECLARED cannot see: a kept path repointed to the live "
               "catalog; a handler put back onto a retired screen, or a thirteenth path repointed "
               "without the contract, turns this red",
    },
    {
        "name": "events_booking.EVENT_SCREENS_MOUNTED_BY_THE_CLIENT_ROUTER ~ routes.rs absence",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "EVENT_SCREENS_MOUNTED_BY_THE_CLIENT_ROUTER",
        "source": "src/ui/routes.rs",
        # A MOUNT is the component followed by its brace; the import list names the screens
        # without one. Zero expected (D16), so the witness proves the scan still sees a mount.
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT
                    + r"\b(?:EventsScreen|EventDetailScreen|MyBookingsScreen)\s*\{"),
        "witness": r"CatalogScreen \{\}",
        "relation": "equal",
        "why": "the owner's ruling took events off every customer surface; a handler that mounts "
               "an events screen again puts the calendar and its booking modal back in front of "
               "every customer holding an old link",
    },
    {
        "name": "events_booking.SCREEN_ROUTES ~ routes.rs events path attributes",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "SCREEN_ROUTES",
        "source": "src/ui/routes.rs",
        "extract": ("regex_all", r'^\s*#\[route\("(/events|/events/:id|/my-bookings)"\)\]'),
        "relation": "list_equal",
        "why": "the three paths stay DECLARED because a link already sent never expires; one "
               "deleted is a router miss for every customer who kept it",
    },
    {
        "name": "events_booking.EVENTS_PUBLISHABLE ~ events.rs EVENTS_PUBLISHABLE",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "EVENTS_PUBLISHABLE",
        "source": "src/api/events.rs",
        "extract": ("regex", r"pub\(crate\) const EVENTS_PUBLISHABLE:\s*bool\s*=\s*(true|false)\s*;"),
        # A `false` reads as zero to the D16 rule, and the value means nothing unless the
        # write path still consults it: the witness pins the gate both validators call.
        "witness": r"fn storable_public_flag\(requested: bool\) -> bool \{\s*requested && EVENTS_PUBLISHABLE\s*\}",
        "relation": "equal",
        "why": "every customer read of an event filters is_public; flipped to true, the admin "
               "API publishes again and the calendar, the booking and the promo sweep reach "
               "customers with no other line changing",
    },
    {
        "name": "events_booking.EVENT_SHARE_REFUSAL_ARMS ~ share.rs event kind answers not found",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "EVENT_SHARE_REFUSAL_ARMS",
        "source": "src/api/share.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT + r"ShareKind::Event => return Ok\(None\)"),
        "witness": r"ShareKind::Strain => return Ok\(None\)",
        "relation": "equal",
        "why": "the event lookup behind the share card has no is_public filter, so building the "
               "card again renders a hidden event for anyone who names its id",
    },
    {
        "name": "deeplink.LEGACY_ROUTE_ALIASES ~ routes.rs compatibility path attributes",
        "spec": "specs/turbobaby/deeplink.t27",
        "const": "LEGACY_ROUTE_ALIASES",
        "source": "src/ui/routes.rs",
        # Presence only, in any order (the contract lists /skate first, the router declares it
        # last). What each path renders is the REPOINTED row's and the ride contracts' business.
        # Seven legacy paths joined on 2026-09-25 (the owner's answer; 6 -> 13 members).
        "extract": ("regex_all", r'^\s*#\[route\("(/skate|/sets|/sommelier|/events|/events/:id|/my-bookings|/accessories|/tea|/game|/treasure-hunt|/ar-hunt|/location-quest|/tech-tree)"\)\]'),
        "relation": "set_equal",
        "why": "LEGACY_LINKS_DO_NOT_EXPIRE: every path in the compatibility set must stay "
               "declared, or links already in customers' Telegram histories become router misses",
    },
    {
        "name": "commerce.SALE_LINE_REFUSAL_ARMS ~ orders.rs validate_bike_lines sale arm",
        "spec": "specs/turbobaby/commerce.t27",
        "const": "SALE_LINE_REFUSAL_ARMS",
        "source": "src/api/orders.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT
                    + r"BikeDeal::BikeSale \{ \.\. \} => return Err\(StatusCode::UNPROCESSABLE_ENTITY\)"),
        "relation": "equal",
        "why": "SALE_LINE_ADMITTED_AT_CHECKOUT is false; without this arm POST /api/orders admits "
               "a sale line for any known family again, with no sale owner asked",
    },
    {
        "name": "catalog_api.PUBLIC_READS_WITHHOLDING_THE_SALE_OFFER ~ bikes.rs public serialisers",
        "spec": "specs/turbobaby/catalog_api.t27",
        "const": "PUBLIC_READS_WITHHOLDING_THE_SALE_OFFER",
        "source": "src/api/bikes.rs",
        # The two public calls spell their listing `l` and `listing`; the unit test beside them
        # spells it `stored` and is not a public read, so it is not counted.
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT
                    + r"(?:family_json|family_detail_json)\(&(?:l|listing)\.with_sale_withheld\(\)"),
        "relation": "equal",
        "why": "the public list and the public detail each serve the family with its sale offer "
               "withheld; one fewer and that read serves the stored flag and asking price again",
    },
    {
        "name": "catalog_api.SALE_BLOCK_KEY_LINES_IN_UI ~ src/ui/ absence",
        "spec": "specs/turbobaby/catalog_api.t27",
        "const": "SALE_BLOCK_KEY_LINES_IN_UI",
        "source": "src/ui/**/*.rs",
        "extract": ("tree_regex_count", NOT_IN_A_LINE_COMMENT + r"\bT_BIKE_SALE_(?:TITLE|PRICE)\b"),
        # Zero expected (D16): the scan must still see the detail screen's Book control.
        "witness": r"T_BIKE_BOOK",
        "relation": "equal",
        "why": "the buy-out heading and price label were the whole customer sale block; a line "
               "that uses either again brings the block back, and the public wire would then be "
               "all that keeps it empty",
    },
    # Added under review the same day: the write gate above cannot reach an event row that is
    # already public, one made public between migration 085 and the ruling. Planted RED on the
    # tree before the fix, where the file does not exist, and on a copy whose WHERE clause was
    # narrowed to one id (count 0 against 1).
    {
        "name": "events_booking.RULING_HIDE_VISIBILITY_UPDATES_ON_EVENTS ~ 088 events unpublish",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "RULING_HIDE_VISIBILITY_UPDATES_ON_EVENTS",
        "source": "migrations/088_unpublish_events.sql",
        # The statement at column zero and whole, so the migration's comments, which name the
        # flag, are not counted, and a narrower WHERE or a DELETE in its place does not match.
        "extract": ("regex_count", r"^UPDATE events SET is_public = FALSE WHERE is_public = TRUE;$"),
        "relation": "equal",
        "why": "the one statement that hides an event made public after 085; without it such a "
               "row is still served by the calendar, the event page, a booking, a waitlist join "
               "and the promo sweeper's event scans, whatever the admin API may now store",
    },
    # The owner's answer of 2026-09-26 on the previous shop's orders (a customer does not see
    # them at all). Measured by hand 2026-09-26: the list asks for 50 shown orders, and three
    # reads by id -- get_order_status, get_order_details and cancel_order -- answer such an
    # order exactly as a missing one, on the owner check's own line. Each planted RED once by
    # hand (51 in the contract; one guard removed from the source).
    {
        "name": "order_presentation.PREVIOUS_SHOP_ORDER_LIST_LIMIT ~ orders.rs shown-orders cap",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "PREVIOUS_SHOP_ORDER_LIST_LIMIT",
        "source": "src/api/orders.rs",
        "extract": ("regex", r"Order::newest_shown_to_customer\(newest_first, &state\.db\.orm, (\d+)\)"),
        "relation": "equal",
        "why": "the list's cap counts only the orders its customer is shown; read through the "
               "old `.limit` again and the previous shop's orders would take its places",
    },
    {
        "name": "order_presentation.PREVIOUS_SHOP_ORDER_BY_ID_READS ~ orders.rs owner-check guards",
        "spec": "specs/turbobaby/order_presentation.t27",
        "const": "PREVIOUS_SHOP_ORDER_BY_ID_READS",
        "source": "src/api/orders.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT
                    + r"\.telegram_id != Some\(tid\) \|\| !Order::shown_to_customer\(&\w+\) \{"),
        "relation": "equal",
        "why": "each read by id answers an order of the previous shop as a missing one on the "
               "owner check's own line; a guard dropped serves that order by id again",
    },
    # The owner's answer of 2026-09-26 on the previous shop's media (stop serving it): the local
    # /uploads read serves only a name a bike's picture references. Measured by hand 2026-09-26:
    # src/main.rs nests served_uploads, and the reference is read from bikes. Each planted RED
    # once by hand (the nest put back to ServeDir; the table renamed in the contract).
    {
        "name": "upload_media.LOCAL_READ_SERVICE_NESTS ~ main.rs /uploads nest",
        "spec": "specs/turbobaby/upload_media.t27",
        "const": "LOCAL_READ_SERVICE_NESTS",
        "source": "src/main.rs",
        "extract": ("regex_count", NOT_IN_A_LINE_COMMENT
                    + r'\.nest_service\("/uploads", api::upload::served_uploads\('),
        "relation": "equal",
        "why": "the one line that serves /uploads; put back to a directory service and every "
               "file on the volume, the previous shop's included, is served to anyone with its name",
    },
    {
        "name": "upload_media.LOCAL_READ_REFERENCE_LOOKUPS_ON_BIKES ~ upload.rs reference lookup",
        "spec": "specs/turbobaby/upload_media.t27",
        "const": "LOCAL_READ_REFERENCE_LOOKUPS_ON_BIKES",
        "source": "src/api/upload.rs",
        "extract": ("regex_count",
                    r'"SELECT EXISTS \(SELECT 1 FROM bikes WHERE image_url = \$1\) AS referenced"'),
        "relation": "equal",
        "why": "the rental data a served name must be referenced by; another table or column "
               "would serve what the rental catalogue never points at",
    },
    # The operator's reversal of 2026-09-26 under the owner's answer 3: the 24-hour event reminder
    # joins public events only. Measured by hand 2026-09-26: one filtered join, in
    # send_event_reminders. Planted RED once by hand (the filter taken off the join).
    {
        "name": "events_booking.REMINDER_FLAG_FILTERED_JOINS ~ events.rs reminder join",
        "spec": "specs/turbobaby/events_booking.t27",
        "const": "REMINDER_FLAG_FILTERED_JOINS",
        "source": "src/api/events.rs",
        "extract": ("regex_count", r"JOIN events e ON e\.id = b\.event_id AND e\.is_public = TRUE \\$"),
        "relation": "equal",
        "why": "every event is hidden since 088 and each one left is the previous shop's; without "
               "the filter the reminder mails its stored title and venue to a seat holder",
    },
    # The welcome credit's sentence without the garden's words (operator, 2026-09-26, under
    # answer 3): the writer and the read-side rule each hold it, and the contract names it.
    # Measured by hand 2026-09-26. Each planted RED once by hand (the garden's words put back in
    # the writer; a different sentence in the rule).
    {
        "name": "legacy_retirement.WELCOME_CREDIT_SENTENCE ~ referrals.rs welcome row",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "WELCOME_CREDIT_SENTENCE",
        "source": "src/db/referrals.rs",
        "extract": ("regex", r'tx_type: Set\("referral_welcome"\.to_string\(\)\),\s*'
                             r'description: Set\(Some\(\s*("[^"]*")\.to_string\(\)'),
        "relation": "equal",
        "why": "the sentence every new welcome credit stores; the garden's words back in it put "
               "the previous shop's mechanic in the bonus history of every invited customer",
    },
    {
        "name": "legacy_retirement.WELCOME_CREDIT_SENTENCE ~ legacy_view.rs served sentence",
        "spec": "specs/turbobaby/legacy_retirement.t27",
        "const": "WELCOME_CREDIT_SENTENCE",
        "source": "src/trios/legacy_view.rs",
        "extract": ("regex", r'pub const WELCOME_CREDIT_SENTENCE: &str = ("[^"]*");'),
        "relation": "equal",
        "why": "the one welcome sentence the bonus history serves; if it drifts from the writer's, "
               "every new welcome credit is withheld again",
    },
)

TREE_EXTRACTORS = (
    "tree_file_count",
    "tree_regex_count",
    "tree_module_count",
    "tree_line_nth",
    "tree_path_nth",
)

# Extractors whose pattern must expose exactly one capture group. Without this check a
# two-group pattern makes `findall` return TUPLES and the reader dies on
# `'tuple' object has no attribute 'strip'` -- exit 1 with a traceback and no binding
# named, which is fail-closed but useless to whoever has to fix it (measured
# 2026-09-21 against `const (MAX_SCORE): i64 = ([0-9_]+);`).
ONE_GROUP_EXTRACTORS = (
    "regex",
    "regex_list",
    "regex_all",
    "regex_group_counts",
    "regex_group_max",
)

# D16: a gate whose input can reach zero must pin a floor. The table is this gate's
# input. Since 2026-09-22 the floor IS the table size, with no slack: deleting a binding
# after measuring that its two sides stopped being the same fact is still legal, but it
# now means lowering this number in the same change, where a reviewer sees it; quietly
# shrinking the table is not. It was 58 against a table of 65, which is not a floor but
# a seven-row hole: every line_count binding bar five could have gone in silence. Then
# 65 against 66, one row of slack -- one binding that could go in silence. Re-measured
# 2026-09-22: the five binding groups of that day took the table to 162 rows over 40
# contracts, and the floor went to 162, not 161. Later the same day the review fixes added
# six rows (DEAL_KIND_COUNT, the two rental band ends, ESCAPE_REPLACEMENTS,
# ROUTES_TESTING_THE_GRANT_CEILING, FAIL_OPEN_CALL_SITE_FILES) over the same 40 contracts:
# 168 rows, floor 168. Re-measured 2026-09-24: the status, cancellation and money families
# and the person-naming resolution added 29 rows (order_presentation among them, bound for
# the first time) over 41 contracts: 197 rows, floor 197. Until then the floor had stayed
# at 168 while the table held 197, a 29-row hole of exactly the kind this comment forbids.
# The same day the leaderboard leak fix bound game_score.IDENTIFIER_DIGIT_RUN: 198 rows.
# Later that day the catalog-honesty change bound availability.CLICK_125_REDIRECT_LABELS_SHOWN
# and availability.CONFIRMING_KEY_LINES_IN_UI (41 contracts still): 200 rows, floor 200.
# Then the owner's Phuket delivery zones (migration 087) added the sixteen delivery_terms
# rows of the delivery zones group, each planted RED once by hand: 216 rows, floor 216.
# Then T27 C3 bound commerce.CLIENT_DEPOSIT_REFUSALS: 217 rows, floor 217.
# Then the spec-hygiene change added ten over the same 41 contracts: the four
# ride-handling constants against assets/game/ride.js, schema-provenance's three census
# figures, nmax-155's rate in pricing-honesty and in rental-terms, and order-money's seed
# line count. 227 rows, floor 227.
# Then the owner's rental-only ruling of 2026-09-24 added nine rows over the same 41
# contracts: legacy-retirement's repointed count, four events-booking facts (the client
# mounts, the three kept paths, the publish gate and the share refusal), deeplink's
# compatibility set, commerce's sale refusal and catalog-api's two sale-offer facts. 236
# rows, floor 236. Review of that change added events-booking's hide of the rows already
# public (migration 088): 237 rows, floor 237.
# Then the owner's decision of 2026-09-25 on CLICK 125's redirect (NMAX 155 alone, for now)
# bound availability.CLICK_125_REDIRECTS and CLICK_125_REDIRECTS_ARE_PROVISIONAL to the seed,
# each planted RED once by hand: 239 rows over the same 41 contracts, floor 239.
# Then the owner's answer 1 of the second list on 2026-09-25 (the 20+ box removed, for now)
# bound checkout_contact.BLOCKER_COUNT and AGE_FIELD_COMPARISONS_IN_THE_ORDERS_MODULE,
# each planted RED once by hand: 241 rows over the same 41 contracts, floor 241.
# Then the owner's answer 3 of 2026-09-25 (the previous shop's stored content out of
# customers' sight) bound order-presentation's RETIRED_LINE_NAME_FIELD and
# RETIRED_LINE_KEPT_FIELDS and legacy-retirement's OWNER_ANSWER_3_DESCRIBED_TX_TYPE_COUNT and
# OWNER_ANSWER_3_WITHHELD_TX_TYPE_COUNT to src/trios/legacy_view.rs, each planted RED once by
# hand: 243 rows over the same 41 contracts and 81 source files on its own branch, floor 243.
# Merged 2026-09-26 on the integration branch: 245 rows, floor 245 (the measured table size).
# Then the kept-cart change of 2026-09-26 bound cart_persistence.SERVED_CART_KINDS to the shared
# list in src/trios/pricing.rs and REMINDER_KIND_FILTERED_QUERIES to the reminder, each planted
# RED once by hand: 241 rows over the same 41 contracts on its own branch, floor 241. Merged
# 2026-09-26 on the integration branch: 247 rows, floor 247 (the measured table size).
# Then the held notification kinds of 2026-09-26 bound notification_queue.WRITTEN_KINDS to
# the producers' insert_queue_row calls (and re-pointed RENDERABLE_KINDS at
# DeliverableKind::of with a witness): 240 rows over the same 41 contracts on its own branch,
# floor 240. Merged 2026-09-26 on the integration branch: 248 rows, floor 248 (the measured
# table size).
# Then the owner's answer of 2026-09-26 on the previous shop's orders bound
# order_presentation.PREVIOUS_SHOP_ORDER_LIST_LIMIT and PREVIOUS_SHOP_ORDER_BY_ID_READS to the
# order handlers, each planted RED once by hand: 250 rows, floor 250.
# Then the owner's answer of 2026-09-26 on the previous shop's media bound
# upload_media.LOCAL_READ_SERVICE_NESTS to the /uploads nest in src/main.rs and
# LOCAL_READ_REFERENCE_LOOKUPS_ON_BIKES to the reference lookup, each planted RED once by hand: 252 rows,
# floor 252.
# Then the operator's reversal of 2026-09-26 bound events_booking.REMINDER_FLAG_FILTERED_JOINS to
# the reminder's join, planted RED once by hand: 253 rows, floor 253.
# Then the welcome credit's sentence of 2026-09-26 bound legacy_retirement.WELCOME_CREDIT_SENTENCE
# to its writer in src/db/referrals.rs and to the rule in src/trios/legacy_view.rs, each planted
# RED once by hand: 255 rows, floor 255.
# Then the bike line's name of 2026-09-26 bound order_presentation.BIKE_LINE_NAME_FIELDS to the
# rule's own list in src/trios/order_line.rs, planted RED once by hand (the contract's two fields
# swapped): 249 rows over the same 41 contracts and 84 source files on its own branch, floor 249.
# Merged 2026-09-26 on the integration branch (round 4): 256 rows, floor 256 (the measured table
# size: 248 + the server lane's 7 + the client lane's 1).
MIN_BINDINGS = 256


# ---------------------------------------------------------------------------------
# Reading the contract side
# ---------------------------------------------------------------------------------


class Failure(RuntimeError):
    """A binding that is red. Carries the finished message."""


def spec_constant(spec_text: str, spec_rel: str, name: str) -> object:
    """Read one top-level `pub const NAME : TYPE = VALUE;` out of a .t27 file.

    Located by NAME, anchored at column zero so a same-named constant inside a test
    block or a comment cannot answer for it. Arrays span lines, so the value runs to
    the first `;`.
    """
    pattern = re.compile(
        r"^pub const " + re.escape(name) + r"\s*:\s*[^=;]+=\s*(.*?);\s*$",
        re.MULTILINE | re.DOTALL,
    )
    found = pattern.findall(spec_text)
    if len(found) != 1:
        # "missing" is the wrong word for a constant declared twice, and the count is
        # the whole diagnosis, so lead with what was actually found.
        headline = "contract constant missing" if not found else "contract constant declared more than once"
        raise Failure(
            f"{headline}: {spec_rel} declares "
            f"`pub const {name}` {len(found)} times at top level, expected exactly 1"
        )
    return parse_t27_value(found[0].strip(), f"{spec_rel}:{name}")


def parse_t27_value(raw: str, where: str) -> object:
    if raw.startswith("["):
        if not raw.endswith("]"):
            raise Failure(f"{where}: array literal does not close: {raw[:60]!r}")
        return [literal_element(part, where) for part in split_elements(raw[1:-1])]
    return literal_element(raw, where)


def literal_element(raw: str, where: str) -> object:
    raw = raw.strip()
    if len(raw) >= 2 and raw[0] == raw[-1] and raw[0] in "\"'":
        return raw[1:-1]
    if raw in ("true", "false"):
        return raw == "true"
    return parse_number(raw, where)


def strip_line_comments(body: str) -> str:
    """Drop `// ...` from a captured Rust list body, respecting quotes.

    src/api/orders.rs:1998 puts `// legacy alias for delivered` inside VALID_STATUSES.
    Without this the comment is split out as an element and read as an identifier.
    Quote-aware so a `//` inside a string literal survives.
    """
    out: list[str] = []
    quote = ""
    index = 0
    while index < len(body):
        ch = body[index]
        if quote:
            out.append(ch)
            if ch == "\\":
                index += 1
                if index < len(body):
                    out.append(body[index])
            elif ch == quote:
                quote = ""
            index += 1
            continue
        if ch in "\"'":
            quote = ch
            out.append(ch)
            index += 1
            continue
        if body.startswith("//", index):
            newline = body.find("\n", index)
            if newline == -1:
                break
            index = newline
            continue
        out.append(ch)
        index += 1
    return "".join(out)


def split_elements(body: str) -> list[str]:
    """Split a list body on commas and `|`, respecting quotes, dropping empties.

    `|` is here for Rust `matches!` arms, which spell the same list with pipes.
    """
    out: list[str] = []
    current: list[str] = []
    quote = ""
    for ch in body:
        if quote:
            current.append(ch)
            if ch == quote:
                quote = ""
            continue
        if ch in "\"'":
            quote = ch
            current.append(ch)
            continue
        if ch in ",|":
            out.append("".join(current))
            current = []
            continue
        current.append(ch)
    out.append("".join(current))
    return [part.strip() for part in out if part.strip()]


def parse_number(raw: str, where: str) -> int | float:
    """Parse an integer or float, including Rust separators and products.

    `100 * 1024 * 1024` and `5 * 60` are how this repository spells two of the bounds
    a contract states as one number; evaluating the product is cheaper and more honest
    than demanding the source be rewritten to match the contract's arithmetic.
    """
    total: int | float = 1
    for part in raw.split("*"):
        token = part.strip().replace("_", "")
        for suffix in ("i64", "i32", "u64", "u32", "u16", "u8", "usize", "f64", "f32"):
            if token.endswith(suffix):
                token = token[: -len(suffix)]
        if re.fullmatch(r"-?\d+", token):
            total *= int(token)
        elif re.fullmatch(r"-?\d+\.\d*", token):
            total *= float(token)
        else:
            raise Failure(f"{where}: {raw!r} is not a number this reader can evaluate")
    if isinstance(total, float) and total.is_integer():
        return int(total)
    return total


# ---------------------------------------------------------------------------------
# Reading the source side
# ---------------------------------------------------------------------------------


def resolve_identifier(
    token: str,
    resolve_in: tuple[str, ...],
    where: str,
    overrides: dict[str, Path] | None = None,
) -> str:
    """Resolve a bare identifier to the `&str` constant it names.

    src/api/bikes.rs builds UNIT_STATUSES out of const references, one of which
    (`UNIT_STATUS_AVAILABLE`) lives in src/db/bikes.rs so that the published count and
    the queried count cannot disagree. Matching the array as text would compare
    identifiers against status names and always fail.

    `overrides` is the --source-override map and it MUST reach here. It did not until
    2026-09-21, and the effect was the one main() calls the worst possible answer: with
    src/api/bikes.rs overridden by a copy whose UNIT_STATUS_RENTED reads "LEASED", the
    gate printed "OK - 65 bindings hold", because the array body came from the copy and
    every identifier in it was resolved against the untouched file on disk. The same
    edit made on disk goes red on sight. An override that half-applies proves the gate
    cannot catch a thing it catches.
    """
    if not resolve_in:
        raise Failure(
            f"{where}: source list holds the bare identifier {token!r} and the binding "
            "names no `resolve_in` files to resolve it against"
        )
    pattern = re.compile(
        r"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?const " + re.escape(token)
        + r"\s*:\s*&(?:'static\s+)?str\s*=\s*\"([^\"]*)\"\s*;"
    )
    hits: list[str] = []
    for rel in resolve_in:
        path = (overrides or {}).get(rel, REPO / rel)
        if not path.is_file():
            raise Failure(f"{where}: resolve_in names {rel}, which does not exist")
        hits.extend(pattern.findall(path.read_text(encoding="utf-8", errors="strict")))
    if len(hits) != 1:
        raise Failure(
            f"{where}: identifier {token!r} resolves to {len(hits)} `&str` constants "
            f"across {list(resolve_in)}, expected exactly 1"
        )
    return hits[0]


def glob_files(pattern: str, where: str) -> list[Path]:
    """Expand a repo-relative glob, sorted, files only.

    A glob that matches nothing is RED, never an empty census: `modules_with_pub_fn_
    routes()` returning an empty vector for three months is the incident DECISIONS.md
    D16 is written from.
    """
    found = sorted(p for p in REPO.glob(pattern) if p.is_file())
    if not found:
        raise Failure(f"{where}: glob {pattern!r} matched no files (D16)")
    return found


def extract_tree(
    binding: dict[str, object], where: str, detail: dict[str, str] | None = None
) -> object:
    """Read a fact out of every file a glob matches.

    `detail`, when given, collects one line saying WHAT was measured, for the failure
    message. A glob-sourced binding otherwise reports "source src/**/*.rs", which for
    the rank extractors is the one thing the reader needs and cannot get.
    """
    kind, *rest = binding["extract"]  # type: ignore[misc]
    files = glob_files(str(binding["source"]), where)

    # The witness is checked HERE, before any early return. It used to live below the
    # two returns above, which meant a `witness` on a tree_file_count or tree_line_nth
    # binding was accepted and never evaluated -- and main()'s D16 rule ("a contract
    # value of 0 must carry a witness") could therefore be satisfied by a pattern
    # nothing ever ran. Verified 2026-09-21: extract_tree returned 87 for a
    # tree_file_count binding carrying a witness that matches nothing in the corpus.
    witness = binding.get("witness")
    if witness is not None:
        if not any(
            re.search(str(witness), p.read_text(encoding="utf-8", errors="strict"))
            for p in files
        ):
            raise Failure(
                f"{where}: witness pattern matched nothing in any of the {len(files)} "
                "files the glob found; an absence is only a measurement while the scan "
                "can still see the things it is absent from (D16)"
            )

    if kind == "tree_file_count":
        if detail is not None:
            detail["measured"] = f"{len(files)} files matched by {binding['source']}"
        return len(files)

    if kind in ("tree_line_nth", "tree_path_nth"):
        rank = int(rest[0])
        ranked = sorted(
            (
                (len(p.read_text(encoding="utf-8", errors="strict").splitlines()),
                 p.relative_to(REPO).as_posix())
                for p in files
            ),
            key=lambda row: (-row[0], row[1]),
        )
        if rank < 1 or rank > len(ranked):
            raise Failure(
                f"{where}: rank {rank} is outside the {len(ranked)} files the glob found"
            )
        lines, rel = ranked[rank - 1]
        if detail is not None:
            detail["measured"] = f"rank {rank} of {len(ranked)} is {rel} at {lines} lines"
        if kind == "tree_line_nth":
            # A tie does not make a LENGTH ambiguous -- "how long is the Nth longest
            # file" has one answer however many files share it -- so tree_line_nth does
            # not refuse one. Only the identity below is ambiguous under a tie.
            return lines
        neighbours = [r for n, r in ranked if n == lines]
        if len(neighbours) > 1:
            raise Failure(
                f"{where}: rank {rank} is a tie at {lines} lines between {neighbours}, "
                "so WHICH file holds it is not a fact this scan can report; the sort "
                "would answer by filename, which is not what the contract claims"
            )
        return rel

    pattern = re.compile(rest[0], re.MULTILINE)
    total = 0
    modules = 0
    for path in files:
        hits = len(pattern.findall(path.read_text(encoding="utf-8", errors="strict")))
        total += hits
        if hits:
            modules += 1
    if detail is not None:
        detail["measured"] = (
            f"{total} match(es) in {modules} of {len(files)} files under {binding['source']}"
        )
    return total if kind == "tree_regex_count" else modules


def extract_source(
    binding: dict[str, object],
    text: str,
    where: str,
    overrides: dict[str, Path] | None = None,
) -> object:
    kind, *rest = binding["extract"]  # type: ignore[misc]

    if kind == "line_count":
        return len(text.splitlines())

    # DOTALL so a list literal may span lines; MULTILINE so `^` means "start of line"
    # and a pattern can insist a Rust attribute sits at column zero.
    pattern = re.compile(rest[0], re.DOTALL | re.MULTILINE)
    if kind in ONE_GROUP_EXTRACTORS and pattern.groups != 1:
        raise Failure(
            f"{where}: the {kind} extractor reads capture group 1, and this pattern "
            f"has {pattern.groups} group(s): {rest[0]!r}"
        )

    if kind == "regex_count":
        return len(pattern.findall(text))

    if kind == "regex_all":
        captures = pattern.findall(text)
        if not captures:
            raise Failure(
                f"source anchor matched nothing: {where} for {rest[0]!r}; an empty "
                "list is not a measurement (D16)"
            )
        return list(captures)

    if kind == "regex_group_counts":
        order_pattern = binding.get("order_by")
        if order_pattern is None:
            raise Failure(f"{where}: regex_group_counts needs an `order_by` pattern")
        order = re.compile(str(order_pattern), re.DOTALL | re.MULTILINE).findall(text)
        if not order:
            raise Failure(
                f"{where}: the `order_by` pattern matched nothing, so there is no key "
                "sequence to count against (D16)"
            )
        if len(order) != len(set(order)):
            raise Failure(f"{where}: the `order_by` pattern yields duplicate keys")
        counts: dict[str, int] = {key: 0 for key in order}
        for key in pattern.findall(text):
            if key not in counts:
                # Silently dropping it would under-count and stay green.
                raise Failure(
                    f"{where}: counted a row keyed {key!r}, which the `order_by` "
                    "sequence does not contain"
                )
            counts[key] += 1
        return [counts[key] for key in order]

    if kind == "regex_group_max":
        counts: dict[str, int] = {}
        for match in pattern.finditer(text):
            counts[match.group(1)] = counts.get(match.group(1), 0) + 1
        if not counts:
            raise Failure(
                f"source anchor matched nothing: {where} found 0 groups for "
                f"{rest[0]!r}; a max over an empty scan is not a measurement (D16)"
            )
        return max(counts.values())

    matches = pattern.findall(text)
    wanted = int(binding.get("occurrences", 1))  # type: ignore[arg-type]
    if len(matches) != wanted:
        raise Failure(
            f"source anchor matched {len(matches)} time(s), expected {wanted}: "
            f"{where} for {rest[0]!r}"
        )

    if kind == "regex_list":
        resolve_in = tuple(binding.get("resolve_in", ()))  # type: ignore[arg-type]
        values: list[str] = []
        for token in split_elements(strip_line_comments(matches[0])):
            if len(token) >= 2 and token[0] == token[-1] and token[0] in "\"'":
                values.append(token[1:-1])
            else:
                values.append(resolve_identifier(token, resolve_in, where, overrides))
        if not values:
            raise Failure(f"source anchor matched an EMPTY list: {where} (D16)")
        return values

    if kind == "regex":
        parsed = [literal_element(m, where) for m in matches]
        if len({repr(v) for v in parsed}) != 1:
            raise Failure(
                f"the {wanted} copies of this bound disagree with each other: "
                f"{where} yields {parsed}"
            )
        return parsed[0]

    raise Failure(f"{where}: unknown extractor {kind!r}")


# ---------------------------------------------------------------------------------
# Relations
# ---------------------------------------------------------------------------------


def compare(relation: str, contract: object, source: object) -> str | None:
    """Return None when the binding holds, or a sentence saying how it does not."""
    # Python says False == 0 and True == 1, so a contract `bool` bound against a count
    # would hold on a number that means nothing like it -- and main()'s D16 rule, which
    # tests `contract_value == 0`, would demand a witness for a `false`. No binding is
    # bool-sided today, but the corpus is full of bool constants (ROUTE_ESCAPES_THE_
    # GENERAL_BODY_LIMIT, SIZE_CAP_PRECEDES_PARSING, FRESHNESS_IS_CHECKED_IN_BOTH_PATHS)
    # and the table is meant to grow into them. Refuse the comparison rather than pass
    # it: a bool and an integer are not the same kind of fact.
    if isinstance(contract, bool) != isinstance(source, bool):
        return (
            f"one side is a bool and the other is not, so these are not the same kind "
            f"of fact: contract={contract!r} source={source!r}"
        )
    if relation == "equal":
        if contract == source:
            return None
        return f"contract={contract!r} source={source!r}"
    if relation == "list_equal":
        if not isinstance(contract, list) or not isinstance(source, list):
            return f"list_equal needs two lists; contract={contract!r} source={source!r}"
        if contract == source:
            return None
        return f"contract={contract!r} source={source!r} (order counts here)"
    if relation == "set_equal":
        if not isinstance(contract, list) or not isinstance(source, list):
            return f"set_equal needs two lists; contract={contract!r} source={source!r}"
        only_contract = sorted(set(map(repr, contract)) - set(map(repr, source)))
        only_source = sorted(set(map(repr, source)) - set(map(repr, contract)))
        if not only_contract and not only_source:
            return None
        return f"only in contract={only_contract} only in source={only_source}"
    if relation in ("source_at_most", "source_at_least"):
        if not isinstance(contract, (int, float)) or not isinstance(source, (int, float)):
            return f"{relation} needs two numbers; contract={contract!r} source={source!r}"
        if relation == "source_at_most" and source <= contract:
            return None
        if relation == "source_at_least" and source >= contract:
            return None
        sign = "<=" if relation == "source_at_most" else ">="
        return f"source={source!r} is not {sign} contract={contract!r}"
    return f"unknown relation {relation!r}"


# ---------------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------------


def read_text(path: Path, rel: str) -> str:
    if not path.is_file():
        raise Failure(f"file named by the binding does not exist: {rel}")
    return path.read_text(encoding="utf-8", errors="strict")


def check_tracked(paths: set[str], problems: list[str]) -> None:
    """Every file a binding reads must be in git.

    Without this a binding can be satisfied by a scratch copy nobody reviews. Mirrors
    `require_tracked` in scripts/verify_t27_specs.py.
    """
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", *sorted(paths)],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        listed = set(
            subprocess.run(
                ["git", "ls-files"],
                cwd=REPO,
                text=True,
                capture_output=True,
                check=False,
            ).stdout.splitlines()
        )
        untracked = sorted(p for p in paths if p not in listed)
        problems.append(f"files read by bindings but not tracked in git: {untracked}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("-v", "--verbose", action="store_true", help="print every binding")
    parser.add_argument(
        "--require-git-tracked",
        action="store_true",
        help="fail when a file a binding reads is not tracked in git; implied in CI",
    )
    parser.add_argument(
        "--source-override",
        action="append",
        default=[],
        metavar="REPO_PATH=OTHER_PATH",
        help="local-only: read one source file from somewhere else. For negative "
             "controls -- plant a disagreement in a copy and point the gate at it. "
             "Forbidden in CI.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    in_ci = os.environ.get("CI", "").lower() in {"1", "true", "yes"}

    overrides: dict[str, Path] = {}
    for item in args.source_override:
        if in_ci:
            print(
                "verify_t27_against_source: FAIL - --source-override is forbidden in CI",
                file=sys.stderr,
            )
            return 1
        if "=" not in item:
            print(
                f"verify_t27_against_source: FAIL - --source-override {item!r} is not "
                "REPO_PATH=OTHER_PATH",
                file=sys.stderr,
            )
            return 1
        rel, _, other = item.partition("=")
        overrides[rel.strip().replace("\\", "/")] = Path(other.strip()).expanduser()

    # The two flags contradict each other. --require-git-tracked vouches for the
    # repo-relative paths in the table; --source-override makes the gate read something
    # else, usually an untracked scratch file outside the repository. Run together they
    # print a green tracked-check over a file nobody can review (measured 2026-09-21:
    # the pair reported src/trios/stars_cap.rs as tracked while reading a planted copy
    # from the session scratchpad). Refuse the combination rather than report a lie.
    if overrides and args.require_git_tracked:
        print(
            "verify_t27_against_source: FAIL - --source-override cannot be combined "
            "with --require-git-tracked: the tracked check would vouch for a file the "
            "gate is not reading",
            file=sys.stderr,
        )
        return 1

    problems: list[str] = []

    # The floor on the table itself, before anything is read.
    if len(BINDINGS) < MIN_BINDINGS:
        problems.append(
            f"the binding table holds {len(BINDINGS)} entries; the floor is "
            f"{MIN_BINDINGS} (DECISIONS.md D16)"
        )

    names = [str(b["name"]) for b in BINDINGS]
    duplicates = sorted({n for n in names if names.count(n) > 1})
    if duplicates:
        problems.append(f"duplicate binding names: {duplicates}")

    file_sources = {
        str(b["source"]) for b in BINDINGS if str(b["extract"][0]) not in TREE_EXTRACTORS  # type: ignore[index]
    }
    glob_sources = {
        str(b["source"]) for b in BINDINGS if str(b["extract"][0]) in TREE_EXTRACTORS  # type: ignore[index]
    }
    # An override that silently applies to nothing would be the worst possible answer
    # in the one situation the flag exists for: proving the gate can go red.
    unknown_override = sorted(set(overrides) - file_sources)
    if unknown_override:
        blocked = [name for name in unknown_override if name in glob_sources]
        problems.append(
            f"--source-override names sources no single-file binding reads: "
            f"{unknown_override}"
            + (f"; {blocked} are tree globs, which cannot be overridden" if blocked else "")
        )

    if args.require_git_tracked or in_ci:
        touched = {str(b["spec"]) for b in BINDINGS} | file_sources
        for pattern in glob_sources:
            try:
                touched.update(
                    p.relative_to(REPO).as_posix()
                    for p in glob_files(pattern, "require-git-tracked")
                )
            except Failure as error:
                problems.append(str(error))
        check_tracked(touched, problems)

    spec_cache: dict[str, str] = {}
    source_cache: dict[str, str] = {}
    held = 0

    for binding in BINDINGS:
        label = str(binding["name"])
        spec_rel = str(binding["spec"])
        source_rel = str(binding["source"])
        const = str(binding["const"])
        kind = str(binding["extract"][0])  # type: ignore[index]
        try:
            if spec_rel not in spec_cache:
                spec_cache[spec_rel] = read_text(REPO / spec_rel, spec_rel)

            contract_value = spec_constant(spec_cache[spec_rel], spec_rel, const)

            # D16 enforced in code rather than in a comment: a binding that expects
            # ZERO is satisfied by a scan that has gone blind, so it must carry a
            # witness proving the scan can still see anything at all.
            if contract_value == 0 and binding.get("witness") is None:
                raise Failure(
                    f"{const} is 0 and this binding declares no `witness`: a zero from "
                    "a scan that cannot see is indistinguishable from a zero that is "
                    "true (DECISIONS.md D16)"
                )

            detail: dict[str, str] = {}
            if kind in TREE_EXTRACTORS:
                source_value = extract_tree(binding, f"{source_rel} ({label})", detail)
            else:
                source_key = f"{source_rel}|{overrides.get(source_rel, '')}"
                if source_key not in source_cache:
                    source_path = overrides.get(source_rel, REPO / source_rel)
                    source_cache[source_key] = read_text(source_path, str(source_path))

                witness = binding.get("witness")
                if witness is not None:
                    if not re.search(str(witness), source_cache[source_key], re.DOTALL):
                        # Two jobs, two messages (split 2026-09-22, when witnesses first
                        # went onto non-zero rows): for a zero it is the D16 floor, for
                        # anything else a shape the value depends on has gone.
                        raise Failure(
                            f"witness pattern matched nothing in {source_rel}: "
                            + (
                                "the binding expects an absence, and an absence is only a "
                                "measurement while the scan can still see the things it "
                                "is absent from (D16)"
                                if contract_value == 0
                                else "the shape this binding's value depends on, which "
                                "the extractor itself cannot see, is gone -- the row's "
                                f"comment names it: {witness!r}"
                            )
                        )

                source_value = extract_source(
                    binding, source_cache[source_key], f"{source_rel} ({label})", overrides
                )
            # A glob-sourced binding otherwise reports only "source src/**/*.rs", which
            # for the rank extractors hides the one fact the reader needs: WHICH file
            # answered. "contract=4599 source=5000" over 183 files is the kind of
            # message that teaches a team to skip the gate.
            measured = f"\n      measured {detail['measured']}" if detail.get("measured") else ""
            verdict = compare(str(binding["relation"]), contract_value, source_value)
            if verdict is not None:
                problems.append(
                    f"{label}: {verdict}\n"
                    f"      contract {spec_rel} :: {const}\n"
                    f"      source   {source_rel}{measured}\n"
                    f"      why      {binding['why']}"
                )
                continue
            held += 1
            if args.verbose:
                print(
                    f"verify_t27_against_source: OK {label}: "
                    f"{const}={contract_value!r} == {source_rel}"
                    + (f" [{detail['measured']}]" if detail.get("measured") else "")
                )
        except Failure as error:
            problems.append(
                f"{label}: {error}\n"
                f"      contract {spec_rel} :: {const}\n"
                f"      source   {source_rel}"
            )
        except OSError as error:
            problems.append(f"{label}: could not read a file: {error}")

    if problems:
        print(
            f"verify_t27_against_source: FAIL - {len(problems)} problem(s) over "
            f"{len(BINDINGS)} binding(s):",
            file=sys.stderr,
        )
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    specs = len({str(b["spec"]) for b in BINDINGS})
    sources = len({str(b["source"]) for b in BINDINGS})
    print(
        f"verify_t27_against_source: OK - {held} bindings hold across {specs} "
        f"contracts and {sources} source files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
