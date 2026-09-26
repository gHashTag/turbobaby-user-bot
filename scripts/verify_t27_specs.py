#!/usr/bin/env python3
"""Verify every manifested TurboBaby .t27 spec with the pinned external compiler.

The manifest below is deliberately reviewable. Recursive discovery fails when a new
canonical spec is not represented there, so growth cannot be silently skipped. Set
``T27C`` or pass ``--t27c``. A developer without the compiler may skip; CI and
``--require-compiler`` fail when it is unavailable.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
CANONICAL_ROOT = REPO_ROOT / "specs" / "turbobaby"
AGENT_CARD = "specs/agents/turbobaby.t27"

# To add a contract: add one entry after measuring its parse declaration floor and its
# test/invariant/bench floor with the pinned compiler. Discovery supplies the tripwire. All 45 floors equal the measured counts as of 2026-09-25.
# 46 contracts since 2026-09-26, when turbobaby/referral-credit (R3) arrived; all 46 floors equal the
# measured counts that day.
# Re-measured 2026-09-25 for the two contracts the owner's CLICK 125 redirect decision changed:
# availability 114 -> 120 declarations and 32 -> 34 checks, order_money 114 -> 115 declarations.
# Re-measured 2026-09-25 again for the two contracts answer 5 of the owner's second list changed
# (the detail's all-taken line and its key removed): availability 120 -> 132 declarations and
# 34 -> 36 checks, locale_policy 136 -> 138 declarations. Merged 2026-09-26 on top of the /start
# catalog line's 129/35, availability measures 141/37.
# Re-measured 2026-09-26 for the kept-cart change (retired kinds stored, never served):
# cart_persistence 213 -> 266 declarations and 43 -> 52 checks.
# Re-measured 2026-09-26 for the two contracts the notification queue's held kinds changed:
# notification_queue 270 -> 306 declarations and 52 -> 54 checks, locale_policy 136 -> 139 and 35 -> 36.
# Re-measured 2026-09-26 for the owner's answer to question I (booking allowed under a seeded zero):
# availability 141 -> 154 declarations and 37 -> 38 checks, locale_policy 144 -> 146 declarations.
# Re-measured 2026-09-26 again for the nineteen cannabis-era keys retired from the bundle:
# legacy_retirement 299 -> 317 declarations and 63 -> 65 checks, locale_policy 146 -> 148.
# Re-measured 2026-09-26 once more for the bike line's name on the order screens:
# order_presentation 606 -> 622 declarations and 77 -> 79 checks.
SPEC_MANIFEST: dict[str, dict[str, str | int]] = {
    AGENT_CARD: {
        "module": "turbobaby-agent",
        "id": "turbobaby/BOT",
        "min_declarations": 38,
        "min_checks": 9,
    },
    "specs/turbobaby/ai_assist.t27": {
        "module": "turbobaby-ai-assist",
        "id": "turbobaby/ai-assist",
        "min_declarations": 223,
        "min_checks": 45,
    },
    "specs/turbobaby/api_surface.t27": {
        "module": "turbobaby-api-surface",
        "id": "turbobaby/api-surface",
        # Raised 2026-09-26 from 172/42 to the measured count (R1-R3, the owner's answers of 2026-09-26): the referral credit's six routes and stubs re-measured the router and the document.
        "min_declarations": 186,
        "min_checks": 42,
    },
    "specs/turbobaby/availability.t27": {
        "module": "availability",
        "id": "turbobaby/availability",
        # Raised 2026-09-25 from 120/34 to the measured count when the bot's /start catalog
        # line lost CLICK 125 (8 declarations and 1 test), and again when answer 5 of the
        # owner's second list removed the detail's all-taken line (12 declarations and 2 tests).
        # Raised 2026-09-26 from 141/37 when the owner's answer to question I (allow booking; a
        # manager confirms availability) took the unit count out of the Book control (12
        # declarations, customer_may_book and 1 test).
        "min_declarations": 154,
        "min_checks": 38,
    },
    "specs/turbobaby/bike_catalog.t27": {
        "module": "bike-catalog",
        "id": "turbobaby/bike-catalog",
        "min_declarations": 97,
        "min_checks": 41,
    },
    "specs/turbobaby/bot_surface.t27": {
        "module": "turbobaby-bot-surface",
        "id": "turbobaby/bot-surface",
        "min_declarations": 251,
        "min_checks": 59,
    },
    "specs/turbobaby/cart_persistence.t27": {
        "module": "turbobaby-cart-persistence",
        "id": "turbobaby/cart-persistence",
        # 266/52 with the kept-cart change of 2026-09-26; 276/54 once that change was
        # reconciled the same day with the owner's answer 3 (one rule for a kept cart).
        "min_declarations": 276,
        "min_checks": 54,
    },
    "specs/turbobaby/catalog_api.t27": {
        "module": "catalog-api",
        "id": "turbobaby/catalog-api",
        "min_declarations": 140,
        "min_checks": 44,
    },
    "specs/turbobaby/catalog_write.t27": {
        "module": "turbobaby-catalog-write",
        "id": "turbobaby/catalog-write",
        # Raised 2026-09-26 from 158/44 to the measured count (R1-R3, the owner's answers of 2026-09-26): src/main.rs, the second largest file, grew by four lines.
        # 159 on 2026-09-26 (R3, client lane): the admin screen's length re-measured after the
        # Loyalty tab's referral sub-tab, the 5910 kept under ADMIN_SCREEN_LINES_BEFORE_2026_09_26.
        # Merged 2026-09-26 on the integration branch (t27/round5), both kept: 160/44, measured.
        "min_declarations": 160,
        "min_checks": 44,
    },
    "specs/turbobaby/checkout_contact.t27": {
        "module": "turbobaby-checkout-contact",
        "id": "turbobaby/checkout-contact",
        # Raised 2026-09-25 from 210/62 to the measured count when the owner removed the 20+ box
        # for now (second list, answer 1): the age blocker's record and the counts before that day.
        # 245 with the two constants gate 3 binds the removal by, the same day.
        "min_declarations": 245,
        "min_checks": 68,
    },
    "specs/turbobaby/client_errors.t27": {
        "module": "turbobaby-client-errors",
        "id": "turbobaby/client-errors",
        # Raised 2026-09-25 from 239 to the measured count when owner question D's sentence
        # landed by delegation (9 declarations; the checks were renamed, not added). Raised
        # again the same day from 248 to 253 when the sentence was reworded to hold for every
        # 409 (5 declarations; no check added). 249 on its own branch the same day: the retired
        # age body code's count before that day (the 20+ box, removed); merged 2026-09-26, 254.
        "min_declarations": 254,
        "min_checks": 58,
    },
    "specs/turbobaby/commerce.t27": {
        "module": "commerce",
        "id": "turbobaby/commerce",
        "min_declarations": 168,
        "min_checks": 57,
    },
    "specs/turbobaby/customer_surface.t27": {
        "module": "turbobaby-customer-surface",
        "id": "turbobaby/customer-surface",
        "min_declarations": 191,
        "min_checks": 44,
    },
    "specs/turbobaby/deeplink.t27": {
        "module": "turbobaby-deeplink",
        "id": "turbobaby/deeplink",
        # Raised 2026-09-25 from 165/45 to the measured count when the owner's answer 7 added
        # the seven legacy paths to the compatibility set.
        "min_declarations": 170,
        "min_checks": 46,
    },
    "specs/turbobaby/delivery_terms.t27": {
        "module": "turbobaby-delivery-terms",
        "id": "turbobaby/delivery-terms",
        # Raised 2026-09-24 from 253/46 to the measured count when the owner's Phuket zone table
        # landed (88 declarations, 10 checks), and again when the spec-hygiene re-read landed.
        "min_declarations": 359,
        "min_checks": 57,
    },
    "specs/turbobaby/deposit_tiers.t27": {
        "module": "deposit-tiers",
        "id": "turbobaby/deposit-tiers",
        "min_declarations": 137,
        "min_checks": 52,
    },
    "specs/turbobaby/events_booking.t27": {
        "module": "turbobaby-events-booking",
        "id": "turbobaby/events-booking",
        # Raised 2026-09-26 from 326/85 when the operator reversed the kept 24-hour reminder for
        # hidden events (REMINDER_REVERSED_AT).
        "min_declarations": 338,
        "min_checks": 87,
    },
    "specs/turbobaby/game_score.t27": {
        "module": "turbobaby-game-score",
        "id": "turbobaby/game-score",
        "min_declarations": 123,
        "min_checks": 34,
    },
    "specs/turbobaby/happy_hour.t27": {
        "module": "turbobaby-happy-hour",
        "id": "turbobaby/happy-hour",
        "min_declarations": 167,
        "min_checks": 41,
    },
    "specs/turbobaby/http_cache.t27": {
        "module": "turbobaby-http-cache",
        "id": "turbobaby/http-cache",
        "min_declarations": 284,
        "min_checks": 62,
    },
    "specs/turbobaby/legacy_retirement.t27": {
        "module": "turbobaby-legacy-retirement",
        "id": "turbobaby/legacy-retirement",
        # Raised 2026-09-25 from 165/44 to the measured count when the owner's answer 7 on the
        # eight legacy paths was recorded (200/49) and, with it, both halves of ruling #12
        # (client 190/48 and server 194/48 on their own); the three together measure 254/57.
        # 266/59 the same day, with the owner's answer on the 20+ age gate (second list, answer 1).
        # Raised the same day to 277/59 on its own branch when the owner's answer 3 (stored
        # content out of customers' sight) was recorded; merged 2026-09-26, the measured 289/61.
        # 299/63 once answer 3 was reconciled the same day with the kept-cart and held-kind
        # changes (its cart path gave way to the kept-cart rule; two left reads closed).
        # 308/65 on 2026-09-26 when the owner's answer on the previous shop's orders was recorded,
        # 312/66 with the answer on its media, 320/67 with the welcome credit's sentence.
        # 317/65 on its own branch the same day, when the cannabis-era copy the bundle still
        # carried was retired (sixteen LEFTOVERS_2026_09_26_* declarations, one test and one
        # invariant). Merged 2026-09-26 on the integration branch (round 4), the measured 338/69; 341/69 after its review (the welcome sentence stored, not served: +4 WELCOME_SENTENCE_* flags, -1 count).
        # Raised 2026-09-26 from 341/69 to the measured count (R1-R3, the owner's answers of 2026-09-26): R1 and R2 recorded, and the welcome credit's writer retired by R3.
        "min_declarations": 360,
        "min_checks": 73,
    },
    "specs/turbobaby/locale_policy.t27": {
        "module": "turbobaby-locale-policy",
        "id": "turbobaby/locale-policy",
        # 131 on the client half of ruling #12 alone and 130/35 on its server half alone; the
        # same day's cancel key (+1) and the deletion (-14) were reconciled in one count, with
        # two declarations naming the +1; the three together measure 136/35. Answer 5 of the
        # owner's second list that day deleted one more key, named by two declarations: 138/35.
        # 138/35 on its own branch too, with the five keys the removed 20+ box used (-5, two
        # declarations naming them); merged 2026-09-26 on the integration branch, 140/35.
        # Answer 3's key added one declaration naming its recorder (137/35 on its own branch);
        # merged 2026-09-26, 141/35.
        # Raised 2026-09-26 to 139/36 on its own branch: the struct surface re-counted after the
        # queue's held kinds took the retired garden message's field (two declarations and one
        # test); merged 2026-09-26 on the integration branch, 144/36.
        # 146/36 on 2026-09-26: question I deleted the Book control's two count reasons, named
        # by two declarations. 148/36 the same day: nineteen cannabis-era keys no mounted screen
        # printed were deleted, named by two declarations.
        # Raised 2026-09-26 from 148/36 to the measured count (R1-R3, the owner's answers of 2026-09-26): migration 089 re-counted in the migration copy.
        # 155/36 the same day on the client lane of
        # R3 (the referral credit): seven referral keys retired and four declared, named by five
        # declarations, with the two counts before it kept under _BEFORE_2026_09_26_R3 names.
        # Merged 2026-09-26 on the integration branch (t27/round5), both kept: 156/36, measured.
        "min_declarations": 156,
        "min_checks": 36,
    },
    "specs/turbobaby/loyalty_ledger.t27": {
        "module": "turbobaby-loyalty-ledger",
        "id": "turbobaby/loyalty-ledger",
        # Raised 2026-09-26 from 206/47 to the measured count (R1-R3, the owner's answers of 2026-09-26): R1 and R2 split the route table, and R3 removed three paired credits.
        "min_declarations": 219,
        "min_checks": 49,
    },
    "specs/turbobaby/market_profile.t27": {
        "module": "turbobaby-market",
        "id": "turbobaby/market",
        "min_declarations": 79,
        "min_checks": 18,
    },
    "specs/turbobaby/notification_queue.t27": {
        "module": "turbobaby-notification-queue",
        "id": "turbobaby/notification-queue",
        # Raised 2026-09-26 from 270/52 to the measured count when the held kinds were recorded.
        # Raised 2026-09-26 from 306/54 to the measured count (R1-R3, the owner's answers of 2026-09-26): friend_ordered and milestone held with the bonuses they announced.
        "min_declarations": 316,
        "min_checks": 55,
    },
    "specs/turbobaby/observability.t27": {
        "module": "turbobaby-observability",
        "id": "turbobaby/observability",
        # Raised 2026-09-26 from 164/40 to the measured count (R1-R3, the owner's answers of 2026-09-26): the milestone metric left with its caller.
        "min_declarations": 165,
        "min_checks": 40,
    },
    "specs/turbobaby/order_money.t27": {
        "module": "turbobaby-order-money",
        "id": "turbobaby/order-money",
        "min_declarations": 115,
        "min_checks": 37,
    },
    "specs/turbobaby/order_presentation.t27": {
        "module": "turbobaby-order-presentation",
        "id": "turbobaby/order-presentation",
        # Raised 2026-09-25 from 570/73 when the owner's answer 3 gave a line of the old
        # catalogue its neutral name and withheld the previous shop's name. Raised 2026-09-26
        # to 606/77 when the operator decided such an order shows no shop label at all. Raised
        # 2026-09-26 to 624/80 when the owner's answer hid the previous shop's orders altogether;
        # 625/80 measured once its by-id guard count was declared (the floor lagged by one in f3ffc98).
        # 622/79 on its own branch the same day: a bike line is named by its bike instead of
        # "Unknown" (thirteen BIKE_LINE_* declarations and the source function, one test and
        # one invariant). Merged 2026-09-26 on the integration branch (round 4), the measured 641/82.
        "min_declarations": 641,
        "min_checks": 82,
    },
    "specs/turbobaby/order_status.t27": {
        "module": "turbobaby-order-status",
        "id": "turbobaby/order-status",
        # Raised 2026-09-26 from 164/31 to the measured count (R1-R3, the owner's answers of 2026-09-26): the bot reject's referral-credit reversal noted.
        "min_declarations": 170,
        "min_checks": 32,
    },
    "specs/turbobaby/person_naming.t27": {
        "module": "turbobaby-person-naming",
        "id": "turbobaby/person-naming",
        "min_declarations": 235,
        "min_checks": 53,
    },
    "specs/turbobaby/pricing_honesty.t27": {
        "module": "pricing-honesty",
        "id": "turbobaby/pricing-honesty",
        # Raised 2026-09-26 from 120/40 to the measured count (R1-R3, the owner's answers of 2026-09-26): migration 089 re-counted in the migration copy.
        "min_declarations": 122,
        "min_checks": 40,
    },
    "specs/turbobaby/promo_broadcast.t27": {
        "module": "turbobaby-promo-broadcast",
        "id": "turbobaby/promo-broadcast",
        "min_declarations": 205,
        "min_checks": 62,
    },
    "specs/turbobaby/publication.t27": {
        "module": "turbobaby-publish-proof",
        "id": "turbobaby/publication",
        "min_declarations": 166,
        "min_checks": 39,
    },
    "specs/turbobaby/rate_limit.t27": {
        "module": "turbobaby-rate-limit",
        "id": "turbobaby/rate-limit",
        "min_declarations": 159,
        "min_checks": 38,
    },
    "specs/turbobaby/referral_credit.t27": {
        "module": "turbobaby-referral-credit",
        "id": "turbobaby/referral-credit",
        # New 2026-09-26 (R3, the owner's answer of that day): the referral credit, 10% of every
        # completed rental of an invited friend, in THB. Floors are the measured counts.
        # Raised 2026-09-26 on the integration branch (t27/round5) from 128/21 to the measured
        # 137/22: the API's one shape pinned by its witnesses (five declarations), the seven
        # retired keys and the ninth sentence the client lane met (three), and one test.
        # Raised 2026-09-26 by the review of round 5 from 137/22 to the measured 148/24: the
        # recorder guard's two identities (eight declarations, recorder_refused, one test and one
        # invariant).
        # Raised 2026-09-26 (round 6) from 148/24 to the measured 155/26: the owner's answer on the
        # base of the 10% (after the discount, the net base as built), five declarations, one test
        # and one invariant.
        "min_declarations": 155,
        "min_checks": 26,
    },
    "specs/turbobaby/referral_program.t27": {
        "module": "turbobaby-referral-program",
        "id": "turbobaby/referral-program",
        # Raised 2026-09-26 from 264/59 to the measured count (R1-R3, the owner's answers of 2026-09-26): R3: no referral credit of this domain, the ladder kept as history.
        "min_declarations": 289,
        "min_checks": 62,
    },
    "specs/turbobaby/rental_terms.t27": {
        "module": "rental-terms",
        "id": "turbobaby/rental-terms",
        "min_declarations": 190,
        "min_checks": 62,
    },
    "specs/turbobaby/request_identity.t27": {
        "module": "turbobaby-request-identity",
        "id": "turbobaby/request-identity",
        # Raised 2026-09-26 from 155/30 to the measured count (R1-R3, the owner's answers of 2026-09-26): the gate census re-measured and referral-credit added as a consumer.
        "min_declarations": 161,
        "min_checks": 30,
    },
    "specs/turbobaby/ride_game.t27": {
        "module": "ride-game",
        "id": "turbobaby/ride-game",
        "min_declarations": 214,
        "min_checks": 65,
    },
    "specs/turbobaby/ride_runtime.t27": {
        "module": "turbobaby-ride-lifecycle",
        "id": "turbobaby/ride-runtime",
        "min_declarations": 74,
        "min_checks": 25,
    },
    "specs/turbobaby/runtime_config.t27": {
        "module": "turbobaby-runtime-config",
        "id": "turbobaby/runtime-config",
        # Raised 2026-09-26 from 206/54 to the measured count (R1-R3, the owner's answers of 2026-09-26): the request notice's spawn and the unread welcome amount.
        "min_declarations": 210,
        "min_checks": 54,
    },
    "specs/turbobaby/schema_provenance.t27": {
        "module": "turbobaby-schema-provenance",
        "id": "turbobaby/schema-provenance",
        # Raised 2026-09-26 from 233/51 to the measured count (R1-R3, the owner's answers of 2026-09-26): migration 089 took the next number.
        "min_declarations": 241,
        "min_checks": 51,
    },
    "specs/turbobaby/star_award.t27": {
        "module": "turbobaby-star-award",
        "id": "turbobaby/star-award",
        "min_declarations": 113,
        "min_checks": 43,
    },
    "specs/turbobaby/upload_media.t27": {
        "module": "turbobaby-upload-media",
        "id": "turbobaby/upload-media",
        # Raised 2026-09-26 from 253/50 to 275/53 when the owner's answer on the previous shop's media
        # limited the local read-back to what the rental data references.
        "min_declarations": 275,
        "min_checks": 53,
    },
    "specs/turbobaby/validation_bounds.t27": {
        "module": "turbobaby-validation-bounds",
        "id": "turbobaby/validation-bounds",
        "min_declarations": 221,
        "min_checks": 60,
    },
    "specs/turbobaby/webapp_bridge.t27": {
        "module": "turbobaby-webapp-bridge",
        "id": "turbobaby/webapp-bridge",
        "min_declarations": 263,
        "min_checks": 61,
    },
}

GENERATORS = ("gen", "gen-c", "gen-rust", "gen-verilog", "gen-verilog-hir")
EMPTY_IMPORT_PATTERNS = (
    re.compile(r"(?m)^\s*const\s+=\s*@import\([\"']\.zig[\"']\)"),
    re.compile(r"(?m)^\s*(?:use|import)\s*;"),
)


class VerificationError(RuntimeError):
    pass


def fail(message: str) -> None:
    raise VerificationError(message)


def repo_relative(path: Path) -> str:
    return path.relative_to(REPO_ROOT).as_posix()


def discover_specs() -> list[Path]:
    discovered = {repo_relative(path) for path in CANONICAL_ROOT.rglob("*.t27")}
    if (REPO_ROOT / AGENT_CARD).is_file():
        discovered.add(AGENT_CARD)
    expected = set(SPEC_MANIFEST)
    if not expected:
        fail("spec manifest is empty")
    if discovered != expected:
        missing = sorted(expected - discovered)
        unmanifested = sorted(discovered - expected)
        fail(
            "spec manifest mismatch: "
            f"missing={missing}, unmanifested={unmanifested}; "
            "add each new spec with module, ID, and measured declaration floors"
        )
    return [REPO_ROOT / relative for relative in sorted(expected)]


def require_ascii(spec: Path) -> None:
    try:
        spec.read_bytes().decode("ascii")
    except UnicodeDecodeError as error:
        fail(f"{repo_relative(spec)}: non-ASCII source at byte {error.start}")


def require_tracked(specs: list[Path], allow_untracked: bool) -> None:
    if allow_untracked:
        if os.environ.get("CI", "").lower() in {"1", "true", "yes"}:
            fail("--allow-untracked is forbidden in CI")
        return
    result = subprocess.run(
        ["git", "ls-files", "--error-unmatch", "--", *map(repo_relative, specs)],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        tracked = set(
            subprocess.run(
                ["git", "ls-files", "--", "specs"],
                cwd=REPO_ROOT,
                text=True,
                capture_output=True,
                check=True,
            ).stdout.splitlines()
        )
        untracked = sorted(repo_relative(spec) for spec in specs if repo_relative(spec) not in tracked)
        fail(f"untracked manifested specs: {untracked}; local-only escape: --allow-untracked")


def resolve_compiler(cli_path: str | None, require: bool) -> Path | None:
    configured = cli_path or os.environ.get("T27C")
    if configured:
        compiler = Path(configured).expanduser().resolve()
        if not compiler.is_file() or not os.access(compiler, os.X_OK):
            fail(f"T27C is not an executable file: {compiler}")
        return compiler
    if require or os.environ.get("CI", "").lower() in {"1", "true", "yes"}:
        fail("T27C is required in CI; set it to the pinned external t27c executable")
    return None


def run(compiler: Path, command: str, spec: Path, *args: str) -> str:
    result = subprocess.run(
        [str(compiler), command, str(spec), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        fail(f"{repo_relative(spec)}: {command} exited {result.returncode}: {detail}")
    if not result.stdout.strip():
        fail(f"{repo_relative(spec)}: {command} produced empty output")
    return result.stdout


def walk_ast(node: Any):
    if isinstance(node, dict):
        yield node
        for value in node.values():
            yield from walk_ast(value)
    elif isinstance(node, list):
        for value in node:
            yield from walk_ast(value)


def extract_id(declarations: list[dict[str, Any]], spec: Path) -> str:
    candidates = [item for item in declarations if item.get("kind") == "ConstDecl" and item.get("name") == "ID"]
    if len(candidates) != 1:
        fail(f"{repo_relative(spec)}: expected exactly one top-level ID, found {len(candidates)}")
    values = [
        child.get("value")
        for child in candidates[0].get("children", [])
        if child.get("kind") == "ExprLiteral" and isinstance(child.get("value"), str)
    ]
    if len(values) != 1 or not values[0].strip():
        fail(f"{repo_relative(spec)}: ID is not one non-empty string literal")
    return values[0]


def verify_spec(compiler: Path, spec: Path) -> tuple[str, str, int, int]:
    relative = repo_relative(spec)
    expected = SPEC_MANIFEST[relative]
    try:
        ast = json.loads(run(compiler, "parse", spec, "--json"))
    except json.JSONDecodeError as error:
        fail(f"{relative}: parse --json was not JSON: {error}")
    module = ast.get("name")
    declarations = ast.get("children")
    if not isinstance(module, str) or not module.strip():
        fail(f"{relative}: parser recovered an empty module name")
    if module != expected["module"]:
        fail(f"{relative}: expected module {expected['module']!r}, got {module!r}")
    if not isinstance(declarations, list):
        fail(f"{relative}: parser returned no declaration list")
    if len(declarations) < int(expected["min_declarations"]):
        fail(
            f"{relative}: only {len(declarations)} declarations; "
            f"baseline is {expected['min_declarations']}"
        )
    for node in walk_ast(ast):
        if node.get("kind") == "UseDecl":
            name = node.get("name")
            value = node.get("value")
            if not isinstance(name, str) or not name.strip() or not isinstance(value, str) or not value.strip():
                fail(f"{relative}: malformed UseDecl with name={name!r}, value={value!r}")
    spec_id = extract_id(declarations, spec)
    if spec_id != expected["id"]:
        fail(f"{relative}: expected ID {expected['id']!r}, got {spec_id!r}")

    try:
        checked = json.loads(run(compiler, "typecheck", spec, "--json"))
    except json.JSONDecodeError as error:
        fail(f"{relative}: typecheck --json was not JSON: {error}")
    if checked.get("ok") is not True or checked.get("errors") != 0 or checked.get("warnings") != 0:
        fail(
            f"{relative}: typecheck failed or warned: ok={checked.get('ok')!r}, "
            f"errors={checked.get('errors')!r}, warnings={checked.get('warnings')!r}, "
            f"messages={checked.get('messages', [])}"
        )

    tests = run(compiler, "test", spec, "--verbose")
    match = re.search(r"Total:\s+(\d+)\s+declarations", tests)
    checks = int(match.group(1)) if match else 0
    if checks < int(expected["min_checks"]):
        fail(f"{relative}: only {checks} test/invariant/bench declarations; baseline is {expected['min_checks']}")

    for generator in GENERATORS:
        output = run(compiler, generator, spec)
        if any(pattern.search(output) for pattern in EMPTY_IMPORT_PATTERNS):
            fail(f"{relative}: {generator} emitted an empty import")
    return module, spec_id, len(declarations), checks


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--t27c", help="path to the external t27c executable")
    parser.add_argument(
        "--require-compiler",
        action="store_true",
        help="fail instead of skipping when no compiler is configured",
    )
    parser.add_argument(
        "--allow-untracked",
        action="store_true",
        help="local-only: verify manifested specs before their first commit",
    )
    parser.add_argument("-v", "--verbose", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        specs = discover_specs()
        for spec in specs:
            require_ascii(spec)
        require_tracked(specs, args.allow_untracked)
        compiler = resolve_compiler(args.t27c, args.require_compiler)
        if compiler is None:
            print("verify_t27_specs: SKIP - set T27C to run the pinned external compiler", file=sys.stderr)
            return 0
        version = subprocess.run(
            [str(compiler), "version"],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if version.returncode != 0 or not version.stdout.strip():
            fail(f"could not read compiler version from {compiler}")
        modules: dict[str, str] = {}
        ids: dict[str, str] = {}
        for spec in specs:
            module, spec_id, declaration_count, check_count = verify_spec(compiler, spec)
            relative = repo_relative(spec)
            if module in modules:
                fail(f"duplicate module {module!r}: {modules[module]} and {relative}")
            if spec_id in ids:
                fail(f"duplicate ID {spec_id!r}: {ids[spec_id]} and {relative}")
            modules[module] = relative
            ids[spec_id] = relative
            if args.verbose:
                print(
                    f"verify_t27_specs: {relative}: module={module}, ID={spec_id}, "
                    f"declarations={declaration_count}, checks={check_count}, "
                    f"generators={len(GENERATORS)}"
                )
        print(
            f"verify_t27_specs: OK - {len(specs)} manifested specs, "
            f"{len(GENERATORS)} generators each; compiler={compiler}"
        )
        return 0
    except VerificationError as error:
        print(f"verify_t27_specs: FAIL - {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
