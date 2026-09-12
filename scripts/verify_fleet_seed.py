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


def fail(msg: str) -> None:
    print(f"verify_fleet_seed: {msg}", file=sys.stderr)
    raise SystemExit(2)


def sql_number(raw: str) -> float | None:
    return None if raw == "NULL" else float(raw)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("-v", "--verbose", action="store_true", help="print what passed")
    args = ap.parse_args()

    for path in (SEED_JSON, SEED_SQL):
        if not path.exists():
            fail(f"{path.relative_to(REPO)} does not exist")

    try:
        seed = json.loads(SEED_JSON.read_text())
    except json.JSONDecodeError as exc:
        fail(f"{SEED_JSON.relative_to(REPO)} is not valid JSON: {exc}")

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
    seen_codes: set[str] = set()

    for row in sql_units:
        key, code = row["key"], row["code"]
        per_family_total[key] += 1
        if row["status"] == "rented":
            per_family_rented[key] += 1
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

    # ---- Report -------------------------------------------------------------
    if problems:
        print(
            f"verify_fleet_seed: {len(problems)} mismatch(es) between "
            f"data/fleet_seed.json and migrations/082_bikes_seed.sql:",
            file=sys.stderr,
        )
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        return 1

    if args.verbose:
        offered = sum(1 for r in sql_families if r["offered"] == "TRUE")
        print(
            f"verify_fleet_seed: OK — {len(sql_families)} families "
            f"({offered} offered), {len(sql_units)} units "
            f"({rented} rented, {available} available); "
            f"{len(price_list_only)} price-list-only families correctly absent"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
