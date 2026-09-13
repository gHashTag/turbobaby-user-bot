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
# test/invariant/bench floor with the pinned compiler. Discovery supplies the tripwire.
SPEC_MANIFEST: dict[str, dict[str, str | int]] = {
    AGENT_CARD: {
        "module": "turbobaby-agent",
        "id": "turbobaby/BOT",
        "min_declarations": 31,
        "min_checks": 7,
    },
    "specs/turbobaby/availability.t27": {
        "module": "availability",
        "id": "turbobaby/availability",
        "min_declarations": 93,
        "min_checks": 26,
    },
    "specs/turbobaby/bike_catalog.t27": {
        "module": "bike-catalog",
        "id": "turbobaby/bike-catalog",
        "min_declarations": 92,
        "min_checks": 40,
    },
    "specs/turbobaby/catalog_api.t27": {
        "module": "catalog-api",
        "id": "turbobaby/catalog-api",
        "min_declarations": 52,
        "min_checks": 20,
    },
    "specs/turbobaby/commerce.t27": {
        "module": "commerce",
        "id": "turbobaby/commerce",
        "min_declarations": 78,
        "min_checks": 28,
    },
    "specs/turbobaby/deposit_tiers.t27": {
        "module": "deposit-tiers",
        "id": "turbobaby/deposit-tiers",
        "min_declarations": 90,
        "min_checks": 39,
    },
    "specs/turbobaby/pricing_honesty.t27": {
        "module": "pricing-honesty",
        "id": "turbobaby/pricing-honesty",
        "min_declarations": 72,
        "min_checks": 26,
    },
    "specs/turbobaby/publication.t27": {
        "module": "turbobaby-publish-proof",
        "id": "turbobaby/publication",
        "min_declarations": 63,
        "min_checks": 21,
    },
    "specs/turbobaby/rental_terms.t27": {
        "module": "rental-terms",
        "id": "turbobaby/rental-terms",
        "min_declarations": 63,
        "min_checks": 28,
    },
    "specs/turbobaby/ride_game.t27": {
        "module": "ride-game",
        "id": "turbobaby/ride-game",
        "min_declarations": 91,
        "min_checks": 33,
    },
    "specs/turbobaby/ride_runtime.t27": {
        "module": "turbobaby-ride-lifecycle",
        "id": "turbobaby/ride-runtime",
        "min_declarations": 63,
        "min_checks": 22,
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
