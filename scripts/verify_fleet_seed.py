#!/usr/bin/env python3
"""Pin migration 082's seed to data/fleet_seed.json.

Two files state the same facts about the same fleet: `data/fleet_seed.json`, which
records where every number came from, and `migrations/082_bikes_seed.sql`, which is
what actually reaches the database. Nothing but habit keeps them equal, and the
failure is silent in the direction that matters -- a price edited in the SQL and not
the JSON is served to customers with a provenance note that no longer describes it.

This script re-derives the comparison. It is not a formatter and not a linter: it
parses both files and asserts field equality, unit counts, and two absences that are
decisions rather than omissions (see DECISIONS.md D9, D12, D14).

    python3 scripts/verify_fleet_seed.py           # quiet unless something is wrong
    python3 scripts/verify_fleet_seed.py -v        # print what passed
    python3 scripts/verify_fleet_seed.py --seed /tmp/planted.json   # negative control

--seed reads another copy of the fleet seed in place of data/fleet_seed.json; the SQL
seed and the market contract are always the tracked ones. It exists so that a planted
copy can show this gate going red (tests/fleet_seed_negative_control.rs, added
2026-09-24: until then nothing had ever seen it fail). A run with --seed says so in
its report, so its OK can never be mistaken for a verdict on the tracked file.

Exit 0 = the two files agree. Exit 1 = they do not, with every mismatch listed.
Exit 2 = a file is missing or unparseable, which is a different failure and is
reported as one rather than being folded into "they disagree".
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import Counter
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SEED_JSON = REPO / "data" / "fleet_seed.json"
SEED_SQL = REPO / "migrations" / "082_bikes_seed.sql"
MARKET_SPEC = REPO / "specs" / "turbobaby" / "market_profile.t27"

# One VALUES row of the families block. The money columns and variant_label are
# `NULL | literal` on purpose: NULL is a value this schema carries meaning in, so the
# pattern has to match it rather than skip the row (D9).
FAMILY_ROW = re.compile(
    r"\('(?P<key>[a-z0-9\-]+)',\s*"
    r"'(?P<brand>[^']*)',\s*"
    r"'(?P<model>[^']*)',\s*"
    r"(?P<variant>NULL|'[^']*'),\s*"
    r"'(?P<cls>scooter|motorcycle)',\s*"
    r"'(?P<body>[^']*)',\s*"
    r"(?P<cc>\d+),\s*"
    r"(?P<rate>NULL|[\d.]+),\s*"
    r"(?P<deposit>NULL|[\d.]+),\s*"
    r"(?P<monthly>NULL|[\d.]+),\s*"
    r"(?P<offered>TRUE|FALSE),\s*"
    r"(?P<sort>\d+)\)",
    re.MULTILINE,
)

UNIT_ROW = re.compile(
    r"\('(?P<key>[a-z0-9\-]+)',\s*"
    r"'(?P<code>[a-z0-9\-]+)',\s*"
    r"(?P<year>NULL|\d+),\s*"
    r"(?P<color>NULL|'[^']*'),\s*"
    r"'(?P<status>available|rented|service|retired)'\)",
    re.MULTILINE,
)

# A unit code is '<family-key>-NN': a slot label. Anything that looks like a plate
# number would be a D14 violation, and the shape is the only cheap guard against one.
UNIT_CODE = re.compile(r"^[a-z0-9\-]+-\d{2}$")

# The market block is a profile against the contract in specs/turbobaby/market_profile.t27
# (D18): exactly 13 contract fields, plus `languages` and `note` as documented extras.
# Unknown keys fail so the shape stays pinned - a field the contract does not know is a
# field nothing checks.
MARKET_CONTRACT_FIELDS = {
    "country_code": ("TH_COUNTRY_CODE", str),
    "country_name": ("TH_COUNTRY_NAME", str),
    "currency_code": ("TH_CURRENCY_CODE", str),
    "currency_minor_digits_iso": ("TH_MINOR_DIGITS_ISO", int),
    "currency_minor_digits_display": ("TH_MINOR_DIGITS_DISPLAY", int),
    "currency_symbol_codepoint": ("TH_SYMBOL_CODEPOINT", int),
    "currency_symbol_placement": ("TH_SYMBOL_PLACEMENT", str),
    "group_separator": ("TH_GROUP_SEPARATOR", str),
    "decimal_separator": ("TH_DECIMAL_SEPARATOR", str),
    "timezone_name": ("TH_TIMEZONE", str),
    "utc_offset_hours": ("TH_UTC_OFFSET_HOURS", int),
    "dst_observed": ("TH_DST", bool),
    "default_calling_code": ("TH_CALLING_CODE", str),
}
MARKET_EXTRA_KEYS = {"languages", "note"}


def spec_const(source: str, name: str) -> str:
    """Read one `pub const NAME : type = VALUE;` literal out of the market spec."""
    match = re.search(rf"pub const {name}\s*:\s*\w+\s*=\s*([^;]+);", source)
    if match is None:
        fail(f"{MARKET_SPEC.relative_to(REPO)} no longer declares {name}")
    return match.group(1).strip()


def check_market_profile(seed: dict, problems: list[str]) -> None:
    """Validate the seed's `market` block against the market contract.

    Three copies of the profile exist (the spec consts, this block, and the Rust
    formatter). Nothing but this check keeps the seed's copy equal to the spec's:
    a market edited here without the contract would drift silently in the
    direction a customer sees.
    """
    market = seed.get("market")
    if not isinstance(market, dict):
        problems.append("market: block is missing - the deployment profile is undeclared (D18)")
        return

    try:
        spec_source = MARKET_SPEC.read_text()
    except OSError:
        fail(f"{MARKET_SPEC.relative_to(REPO)} is not readable")

    unknown = set(market) - set(MARKET_CONTRACT_FIELDS) - MARKET_EXTRA_KEYS
    if unknown:
        problems.append(f"market: keys outside the 13-field contract: {sorted(unknown)}")
    missing = set(MARKET_CONTRACT_FIELDS) - set(market)
    if missing:
        problems.append(f"market: contract fields missing from the block: {sorted(missing)}")

    for field, (const, kind) in MARKET_CONTRACT_FIELDS.items():
        if field not in market:
            continue
        got = market[field]
        literal = spec_const(spec_source, const)
        if kind is bool:
            want = literal == "true"
            if not isinstance(got, bool) or got != want:
                problems.append(f"market.{field}: seed={got!r} market_profile.t27={literal}")
            continue
        if kind is int:
            want = int(literal)
            if not isinstance(got, int) or isinstance(got, bool) or got != want:
                problems.append(f"market.{field}: seed={got!r} market_profile.t27={want}")
            continue
        want = literal.strip('"')
        if not isinstance(got, str) or got != want:
            problems.append(f"market.{field}: seed={got!r} market_profile.t27={want!r}")

    # Shape rules the .t27 language cannot state (no regex there); the bounds it can
    # state are already tested inside the spec itself.
    iso = market.get("currency_minor_digits_iso")
    display = market.get("currency_minor_digits_display")
    if isinstance(iso, int) and isinstance(display, int):
        if not 0 <= display <= iso <= 4:
            problems.append(
                f"market: display {display} / ISO {iso} minor digits violate the "
                "0 <= display <= ISO <= 4 contract"
            )
    if market.get("currency_symbol_codepoint", 0) in range(55296, 57344):
        problems.append("market.currency_symbol_codepoint: inside the Unicode surrogate gap")
    if market.get("group_separator") == market.get("decimal_separator"):
        problems.append("market: group and decimal separators are identical")
    if market.get("currency_symbol_placement") not in ("prefix", "suffix"):
        problems.append(f"market.currency_symbol_placement: {market.get('currency_symbol_placement')!r}")
    for field, pattern in (
        ("country_code", r"^[A-Z]{2}$"),
        ("currency_code", r"^[A-Z]{3}$"),
        ("group_separator", r"^.$"),
        ("decimal_separator", r"^.$"),
        ("timezone_name", r"^[A-Za-z]+(/[A-Za-z0-9_+\-]+)+$"),
        ("default_calling_code", r"^\+\d{1,3}$"),
    ):
        value = market.get(field)
        if isinstance(value, str) and not re.match(pattern, value):
            problems.append(f"market.{field}: {value!r} fails its contract shape")
    offset = market.get("utc_offset_hours")
    if isinstance(offset, int) and not -12 <= offset <= 14:
        problems.append(f"market.utc_offset_hours: {offset} is outside -12..+14")


# Since 2026-09-25 this script also reads the one decision the seed records as a LIST of
# family keys: pricing_policy.not_offered, what a closed family's customer is offered
# instead. Each offered key must be an offered family with a unit the SQL seeds as
# available -- never a price-list-only family, which is what the CLICK 125 list named
# until the owner's decision of that date (DECISIONS.md, the D12 amendments of 2026-09-24
# and 2026-09-25) -- and a non-empty list must name, in offer_instead_source, an entry of
# `sources` that carries a date. The seed is a dated measurement; a decision written into
# it without a dated source is a number whose provenance nobody can check.
#
# This note lives here and not in the module docstring on purpose: the specs cite lines
# of this file (MARKET_SPEC, the unit-row regex, the market field map, the shape
# patterns), and a paragraph added above them would move every one of those citations.
# New code goes below check_market_profile, where no citation points.
ISO_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def check_redirects(
    seed: dict,
    families: dict[str, dict],
    price_list_only: set[str],
    per_family_available: Counter[str],
    problems: list[str],
) -> list[str]:
    """Validate pricing_policy.not_offered: what a closed family's customer is offered.

    Returns one `closed -> offered, ...` label per entry for the verbose report. The
    availability is the SQL seed's `available` status and not the JSON's
    units_available, because the SQL is what reaches the database; the JSON's count is
    already held equal to it above.
    """
    not_offered = seed.get("pricing_policy", {}).get("not_offered")
    if not isinstance(not_offered, dict):
        problems.append("pricing_policy.not_offered: block is missing - D12's redirect is undeclared")
        return []
    sources = seed.get("sources", {})
    labels: list[str] = []
    for closed, entry in sorted(not_offered.items()):
        family = families.get(closed)
        if family is None:
            problems.append(f"{closed}: listed under pricing_policy.not_offered but not a seeded family")
        elif family.get("offered", True):
            problems.append(f"{closed}: listed under pricing_policy.not_offered but seeded as offered")
        offers = entry.get("offer_instead", []) if isinstance(entry, dict) else []
        if not isinstance(offers, list):
            problems.append(f"{closed}.offer_instead: not a list of family keys")
            continue
        for alt in offers:
            if alt in price_list_only:
                problems.append(
                    f"{closed}.offer_instead: {alt} is a price-list-only family - the fleet "
                    "holds zero units of it"
                )
            elif alt not in families or not families[alt].get("offered", True):
                problems.append(f"{closed}.offer_instead: {alt} is not an offered family of the seed")
            elif per_family_available[alt] < 1:
                problems.append(f"{closed}.offer_instead: {alt} has no unit seeded as available")
        if offers:
            named = entry.get("offer_instead_source")
            source = sources.get(named) if isinstance(named, str) else None
            date = source.get("date") if isinstance(source, dict) else None
            if not isinstance(date, str) or not ISO_DATE.match(date):
                problems.append(
                    f"{closed}.offer_instead: offer_instead_source={named!r} names no entry of "
                    "`sources` with a YYYY-MM-DD date - a decision in this file needs a dated source"
                )
        labels.append(f"{closed} -> {', '.join(offers) or 'nothing'}")
    return labels


def fail(msg: str) -> None:
    print(f"verify_fleet_seed: {msg}", file=sys.stderr)
    raise SystemExit(2)


def sql_number(raw: str) -> float | None:
    return None if raw == "NULL" else float(raw)


def shown(path: Path) -> str:
    """Repo-relative when the path is inside the repository, as given otherwise."""
    try:
        return path.resolve().relative_to(REPO).as_posix()
    except ValueError:
        return str(path)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-v", "--verbose", action="store_true", help="print what passed")
    ap.add_argument(
        "--seed",
        type=Path,
        default=SEED_JSON,
        metavar="PATH",
        help="read this fleet seed JSON instead of data/fleet_seed.json (negative control)",
    )
    args = ap.parse_args()
    seed_json: Path = args.seed

    for path in (seed_json, SEED_SQL):
        if not path.exists():
            fail(f"{shown(path)} does not exist")

    try:
        seed = json.loads(seed_json.read_text())
    except json.JSONDecodeError as exc:
        fail(f"{shown(seed_json)} is not valid JSON: {exc}")

    sql = SEED_SQL.read_text()
    if "-- Families" not in sql or "-- Units" not in sql:
        fail(
            f"{SEED_SQL.relative_to(REPO)} has no '-- Families' / '-- Units' section "
            "headers; this script locates the two VALUES blocks by them"
        )

    families_block, units_block = sql.split("-- Units", 1)
    families_block = families_block.split("-- Families", 1)[1]

    families = {f["key"]: f for f in seed["families"]}
    price_list_only = {f["key"] for f in seed["price_list_only"]["families"]}

    problems: list[str] = []

    # ---- Families -----------------------------------------------------------
    sql_families = [m.groupdict() for m in FAMILY_ROW.finditer(families_block)]
    sql_keys = {r["key"] for r in sql_families}

    if len(sql_families) != len(sql_keys):
        dupes = [k for k, n in Counter(r["key"] for r in sql_families).items() if n > 1]
        problems.append(f"duplicate family keys in SQL: {sorted(dupes)}")

    for row in sql_families:
        key = row["key"]
        want = families.get(key)
        if want is None:
            problems.append(f"{key}: seeded in SQL but absent from fleet_seed.json")
            continue
        for field, got, expected in (
            ("brand", row["brand"], want["brand"]),
            ("class", row["cls"], want["class"]),
            ("displacement_cc", int(row["cc"]), want["displacement_cc"]),
            ("base_rate_thb_day", sql_number(row["rate"]), want["base_rate_thb_day"]),
            ("deposit_thb", sql_number(row["deposit"]), want["deposit_thb"]),
            (
                "monthly_low_season_thb",
                sql_number(row["monthly"]),
                want["monthly_low_season_thb"],
            ),
            ("offered", row["offered"] == "TRUE", want.get("offered", True)),
        ):
            if got != expected:
                problems.append(f"{key}.{field}: SQL={got!r} fleet_seed.json={expected!r}")

    for key in sorted(set(families) - sql_keys):
        problems.append(f"{key}: in fleet_seed.json but never seeded")

    # An absence that is a decision: a family the tariff prices and the fleet holds
    # zero of must NOT be in the catalog, or a customer is quoted a bike nobody can
    # hand over.
    for key in sorted(price_list_only & sql_keys):
        problems.append(
            f"{key}: price-list-only family seeded into `bikes` — the fleet holds "
            "zero units of it"
        )

    # ---- Units --------------------------------------------------------------
    sql_units = [m.groupdict() for m in UNIT_ROW.finditer(units_block)]
    per_family_total: Counter[str] = Counter()
    per_family_rented: Counter[str] = Counter()
    per_family_available: Counter[str] = Counter()
    seen_codes: set[str] = set()

    for row in sql_units:
        key, code = row["key"], row["code"]
        per_family_total[key] += 1
        if row["status"] == "rented":
            per_family_rented[key] += 1
        if row["status"] == "available":
            per_family_available[key] += 1
        if code in seen_codes:
            problems.append(f"duplicate unit_code {code}")
        seen_codes.add(code)
        if not UNIT_CODE.match(code):
            problems.append(f"unit_code {code!r} is not the '<family-key>-NN' slot shape")
        if not code.startswith(f"{key}-"):
            problems.append(f"unit_code {code!r} is not prefixed by its family {key!r}")

    for key, want in families.items():
        if per_family_total[key] != want["units_total"]:
            problems.append(
                f"{key}: {per_family_total[key]} units seeded, "
                f"fleet_seed.json says {want['units_total']}"
            )
        if per_family_rented[key] != want["units_rented"]:
            problems.append(
                f"{key}: {per_family_rented[key]} rented, "
                f"fleet_seed.json says {want['units_rented']}"
            )

    totals = seed["totals"]
    rented = sum(per_family_rented.values())
    available = len(sql_units) - rented
    for label, got, expected in (
        ("unit count", len(sql_units), totals["units"]),
        ("rented count", rented, totals["units_rented"]),
        ("available count", available, totals["units_available"]),
        ("family count", len(sql_families), totals["families_in_stock"]),
    ):
        if got != expected:
            problems.append(f"{label}: SQL={got} fleet_seed.json={expected}")

    # ---- Redirects ----------------------------------------------------------
    redirects = check_redirects(seed, families, price_list_only, per_family_available, problems)

    # ---- Market profile -----------------------------------------------------
    check_market_profile(seed, problems)

    # ---- Report -------------------------------------------------------------
    if problems:
        print(
            f"verify_fleet_seed: {len(problems)} mismatch(es) between "
            f"{shown(seed_json)}, migrations/082_bikes_seed.sql "
            f"and the market contract:",
            file=sys.stderr,
        )
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    if args.verbose:
        offered = sum(1 for r in sql_families if r["offered"] == "TRUE")
        market = seed.get("market", {})
        whose = (
            ""
            if seed_json.resolve() == SEED_JSON.resolve()
            else f" (seed read from {shown(seed_json)}, NOT the tracked data/fleet_seed.json)"
        )
        print(
            f"verify_fleet_seed: OK — {len(sql_families)} families "
            f"({offered} offered), {len(sql_units)} units "
            f"({rented} rented, {available} available); "
            f"{len(price_list_only)} price-list-only families correctly absent; "
            f"redirects {', '.join(redirects) or 'none'} name only offered families "
            f"with a unit available, each from a dated source; "
            f"market profile {market.get('country_code')}/{market.get('currency_code')} "
            f"matches specs/turbobaby/market_profile.t27{whose}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
