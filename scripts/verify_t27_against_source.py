#!/usr/bin/env python3
"""Fail the build when a .t27 contract and the Rust or SQL it constrains disagree.

43 contracts live under specs/. Exactly one of them was mechanically tied to the code
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

WHAT A GREEN RUN DOES NOT PROVE. Re-measured 2026-09-21 11:2x, after an adversarial
pass: the table's 66 bindings cover 64 distinct (contract, constant) pairs out of 4220
top-level `pub const` declarations across the 43 files under specs/, touching 16 of
those 43 contracts. That is 1.5% of the declared constants, and it is 0% of the 7890
`assert` statements -- this script compares DECLARED VALUES, it does not execute a
single contract predicate, and a contract whose constants all match the code can still
assert something false about them. Every binding below was measured by hand before it
was written; the rest of the corpus is unchecked and is not claimed otherwise. Growing
the table is the point; a number here that nobody re-measured is the defect this file
exists to catch. (The corpus moves: the same figure read 7921 two hours earlier, before
a sibling wave committed to specs/. Any count in this header is a reading, not a
standing fact.)

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
#              four tree_* extractors, a repo-relative glob.
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
#   resolve_in    files in which bare identifiers found inside a list may be resolved
#                 to `const IDENT: &str = "...";`. Exactly one definition must exist.
#   order_by      required by ("regex_group_counts", ...): a pattern whose capture 1,
#                 in file order, is the key sequence the counts are emitted against.
#
# To add a binding: measure both sides by hand FIRST, confirm they are the same fact
# and not two facts with similar names, then add the row. A wrong binding is worse
# than a missing one.

# One seeded unit row of migrations/082_bikes_seed.sql, in the shape
# scripts/verify_fleet_seed.py:47-56 already uses: (family, code, year, colour,
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
# One families-block row, key captured. migrations/082 seeds families before units, so
# this is also the order availability.t27's four parallel arrays are written in.
FAMILY_ROW_SQL = (
    r"\('([a-z0-9\-]+)',\s*'[^']*',\s*'[^']*',\s*(?:NULL|'[^']*'),\s*"
    r"'(?:scooter|motorcycle)',"
)

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
        "why": "catalog_write.t27:178 records that the COLUMN has no displacement "
               "ceiling, so this API constant is the only one there is; the contract "
               "is the only statement of what it is",
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
        "why": "an inline ceiling with no name of its own -- catalog_write.t27:272 "
               "counts seven such sites, and the contract is where they are written "
               "down at all",
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
    # pricing_honesty.t27:330 OWNS the migration count; locale_policy.t27:173 carries a
    # named copy of it. Binding both to the same directory pins the copy to the owner
    # without either file having to read the other.
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
        "why": "the contract splits 87 into 86 + 1 precisely because a top-level-only "
               "count would have been 86 while claiming to have looked everywhere",
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
    # --- "wc -l over the tracked sources": measurements contracts state in so many words ---------
    # Five contracts open by recording how long the files they describe are, each with
    # the sentence that it is a measurement. api_surface.t27:101, client_errors.t27:94,
    # events_booking.t27:106, customer_surface.t27:387 and catalog_write.t27:103. Every
    # census that follows in those files was taken against a file of that length, so a
    # changed length means the census was taken against a different file.
    # catalog_write.t27:101-104 makes TWO claims, and binding one of them to a RANK was
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
        # The path is not spelled here by choice: catalog_write.t27:98 declares it as
        # ADMIN_SCREEN, and the binding below pins that declaration to the tree, so the
        # string this row reads is the one the contract is judged on.
        "source": "src/ui/screens/admin_screen.rs",
        "extract": ("line_count",),
        "relation": "equal",
        "why": "catalog_write.t27:103-116 argues from the size of this file that no "
               "contract owns its write bounds; the argument is only as current as the "
               "measurement it opens with",
    },
    {
        "name": "catalog_write.ADMIN_SCREEN ~ largest file under src/",
        "spec": "specs/turbobaby/catalog_write.t27",
        "const": "ADMIN_SCREEN",
        "source": "src/**/*.rs",
        # The other half of the sentence at :101-102, "The screen is the largest file".
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
        "why": "events_booking.t27:106 records it so 'the whole file' is a measurement "
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
        # Column-zero anchoring is the cheap stand-in for "attribute POSITION" that
        # customer_surface.t27:394-401 spells out: the four extra hits the naive grep
        # finds sit inside string literals and a comment, all indented.
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
# input. One row of slack, so deleting a binding after measuring that its two sides
# stopped being the same fact is legal; quietly shrinking the table is not. It was 58
# against a table of 65, which is not a floor but a seven-row hole: every line_count
# binding bar five could have gone in silence.
MIN_BINDINGS = 65


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

    src/api/orders.rs:1871 puts `// legacy alias for delivered` inside VALID_STATUSES.
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
                        raise Failure(
                            f"witness pattern matched nothing in {source_rel}: the "
                            "binding expects an absence, and an absence is only a "
                            "measurement while the scan can still see the things it "
                            "is absent from (D16)"
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
