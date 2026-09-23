#!/usr/bin/env python3
"""Execute every assertion in the .t27 corpus, instead of only compiling it.

WHY THIS EXISTS. scripts/verify_t27_specs.py runs the pinned external compiler and
checks STRUCTURE: module name, unique ID, declaration floors, a clean typecheck, and
non-empty output from five generators. None of that reads a single `assert`. Measured
2026-09-21 with the pinned compiler (gHashTag/t27 @ 40003ed1379c8a417e13e45843de35088b88f8c0):

    t27c parse specs/turbobaby/rental_terms.t27 --json

returns `{"kind": "TestBlock", "name": "...", ..., "children": []}` and the same empty
`children` for every `InvariantBlock`, while every `FnDecl` carries its full body. The
parser keeps function bodies and discards test and invariant bodies, so to the whole
toolchain the corpus's assertions are comments. A false one ships under a green build.

So this gate owns the other half: it parses each spec itself and RUNS the assertions.

TWO FRONT ENDS ON PURPOSE. The compiler cannot be the only reader here -- it does not
hand back the statements this gate has to execute -- so this file carries its own lexer
and recursive-descent parser. That is a feature: where the two disagree, one is wrong.
Whenever a compiler is configured (T27C or --t27c; --no-crosscheck opts out) the two
readings are compared per file: every declaration name, the value of the ID constant,
and -- on the part where they do overlap -- every function body, node for node. The run
prints how many of those agreed.

That comparison is not decoration. It is what found the four functions listed in
KNOWN_FRONTEND_DISAGREEMENTS below, whose bodies t27c silently truncates and then emits
as stubs in C and Rust while `typecheck --json` still answers zero errors and zero
warnings. Every run prints those four on stderr, green or not, because a known defect
that stops being visible is a defect nobody fixes.

HOUSE RULES OBSERVED. Python 3 standard library only -- scripts/verify_fleet_seed.py
states the reason in its own header: no toolchain, no new dependency. ASCII source,
English comments, and every non-obvious fact cited by file and line.

FAIL CLOSED (DECISIONS.md D16, lines 235-262: a gate whose input can reach zero must pin
a floor). Four separate guards, because a silent skip is exactly the defect this gate
exists to prevent:
  1. MIN_SPEC_FILES  -- discovery that finds too few specs is a red build.
  2. MIN_ASSERT_LINES -- a text scan that finds too few asserts is a red build, even if
     every assert it did find passed.
  3. Exact site coverage -- every assert line the text scanner found must actually have
     been executed. An evaluator that quietly skipped a block it could not parse, or a
     test body that is never reached, turns red rather than green.
  4. No empty test block -- a `test` that contains no assert at all is a red build. It
     is the one gutting neither gate could see: t27c's AST for an emptied test body is
     identical to a full one, so scripts/verify_t27_specs.py's block counts cannot
     notice, and an assert that is not there is not scanned, so guard 3 cannot either.
     Measured, see run_spec.

WHAT THESE FLOORS DO NOT CLAIM. 1 and 2 are floors, not equalities, which is what D16
asks for ("the floor is well below the real 19 so that deleting a module is not a
failure", DECISIONS.md:257-258). Measured 2026-09-21: dropping the three largest specs
-- 819 assert lines -- leaves 40 files and 7071 assert lines, and this gate stays green.
That class is caught by scripts/verify_t27_specs.py, whose SPEC_MANIFEST is an exact set
and whose per-spec min_checks sit ON the measurement rather than below it. The pair
covers it; this half does not, and says so rather than implying otherwise.

    python3 scripts/execute_t27_assertions.py            # one receipt line, or failures
    python3 scripts/execute_t27_assertions.py -v         # a line per spec, plus totals
    python3 scripts/execute_t27_assertions.py --file specs/turbobaby/rental_terms.t27

Exit 0 = every assertion in the corpus was executed and held. Exit 1 = an assertion
failed, a spec could not be fully evaluated, a floor was breached, or the two front ends
disagree.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SPECS_ROOT = REPO / "specs"

# --- D16 floors -------------------------------------------------------------------
# Measured over the tracked corpus on 2026-09-21: 43 files (42 under specs/turbobaby/
# plus specs/agents/turbobaby.t27) carrying 7888 assert statements. Both floors sit
# deliberately BELOW the measurement, so deleting a spec is not a failure while LOSING
# THE ABILITY TO SEE specs is -- that is the distinction D16 draws at DECISIONS.md:258.
MIN_SPEC_FILES = 40
MIN_ASSERT_LINES = 7000

# --- evaluator limits -------------------------------------------------------------
# Bounded rather than trusted: a spec that recurses or loops forever must produce a red
# build, not a hung CI job. Re-measured 2026-09-22 by running every spec through this
# evaluator with the call stack and each loop's iterations recorded: the deepest real call
# chain is 5 nested calls (specs/turbobaby/order_money.t27, order_money_verdict down to
# order_money_clamp_at_zero), and the longest real loop runs 16 iterations
# (specs/turbobaby/bot_surface.t27's undiscoverable_command_count, which walks all sixteen
# COMMANDS). Until that date this comment said 3 frames and 14 iterations, citing the loops
# of deposit_tiers.t27 by a line number that was right at f4d84a1 and one off by a33e500;
# those loops, in families_on_tier and published_rows_at_amount, run 14, and COMMANDS was
# already sixteen long when the 14 was written. They are cited by function name now, because
# edits above them keep moving the lines. Both limits below sit far above both readings.
MAX_CALL_DEPTH = 64
MAX_LOOP_ITERATIONS = 100000

# Integer widths are REAL BOUNDS, not decoration. A value that leaves the range of its
# declared type is reported as a failure and never silently wrapped: the corpus is
# arithmetic about small quantities -- unit counts, baht, basis points -- and a value
# that does not fit the width the author declared means the spec is wrong.
INT_RANGES = {
    "u8": (0, 255),
    "u16": (0, 65535),
    "u32": (0, 4294967295),
    "u64": (0, 18446744073709551615),
    "i8": (-128, 127),
    "i16": (-32768, 32767),
    "i32": (-2147483648, 2147483647),
    "i64": (-9223372036854775808, 9223372036854775807),
}
SCALAR_TYPES = set(INT_RANGES) | {"bool", "str"}

KEYWORDS = {
    "module", "pub", "const", "var", "fn", "test", "invariant", "struct", "packed",
    "if", "else", "while", "return", "assert", "and", "or", "true", "false",
}

# Longest match first, so "==" never lexes as two "=" tokens.
OPERATORS = (
    "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=",
    "=", "<", ">", "+", "-", "*", "/", "%",
    "(", ")", "{", "}", "[", "]", ",", ":", ".", ";", "!",
)

BACKSLASH = chr(92)


class SpecError(Exception):
    """A spec could not be lexed, parsed or fully evaluated. Always a red build."""

    def __init__(self, path, line, message):
        super().__init__(message)
        self.path = path
        self.line = line
        self.message = message

    def __str__(self):
        return "%s:%s: %s" % (rel(self.path), self.line, self.message)


def rel(path):
    try:
        return Path(path).resolve().relative_to(REPO).as_posix()
    except ValueError:
        return str(path)


# ======================================================================================
# 1. The assert-line scanner -- the subject of the D16 floor
# ======================================================================================
#
# Deliberately NOT built on the lexer below. Its whole job is to say, from the raw text
# and nothing else, how many assertions the corpus contains, so that the parser cannot
# be graded by a counter it also produced. The duplication between this loop and
# Lexer.tokens is the point, not an oversight.
#
# STATED HONESTLY, because the coverage equality rests on it: the two loops are separate
# code, but they are not independent about COMMENTS. The three lines that decide whether
# a line is a comment -- `stripped = raw.strip()` and the `startswith(";") or
# startswith("//")` test -- are character for character the same in both. So a line the
# two agree to ignore is invisible to the equality by construction: prefixing an assert
# with '; ' removes it from the scan and from the parse alike, and the run stays green
# one assertion smaller. That case is bounded by MIN_ASSERT_LINES and by nothing else.
#
# Comment rules, measured over all 43 files: the corpus uses `//` to end of line and a
# leading `;` to end of line, and `;` also terminates a statement. The two `;` roles are
# separated by position -- a `;` that is the first non-whitespace character of a line
# opens a comment; anywhere else it terminates a statement. That is safe because on
# every one of the 19714 code lines the first statement-terminating `;` is the last
# non-whitespace character of its line (0 exceptions, measured 2026-09-21), and the
# lexer below turns any future exception into a parse error rather than a guess.

def scan_assert_lines(path, text):
    """Return the sorted line numbers that carry an `assert` statement.

    Strings are blanked before comments are looked for, so a `//` inside a string
    literal -- and the 1152 `str` constants in the corpus carry English prose -- cannot
    truncate a line of real code.
    """
    lines = []
    for number, raw in enumerate(text.splitlines(), 1):
        stripped = raw.strip()
        if not stripped or stripped.startswith(";") or stripped.startswith("//"):
            continue
        cleaned = []
        in_string = False
        escaped = False
        index = 0
        while index < len(raw):
            char = raw[index]
            if in_string:
                if escaped:
                    escaped = False
                elif char == BACKSLASH:
                    escaped = True
                elif char == '"':
                    in_string = False
                cleaned.append(" ")
                index += 1
                continue
            if char == '"':
                in_string = True
                cleaned.append(" ")
                index += 1
                continue
            if char == "/" and raw[index + 1:index + 2] == "/":
                break
            cleaned.append(char)
            index += 1
        if in_string:
            raise SpecError(path, number, "string literal is not closed before end of line")
        code = "".join(cleaned).strip()
        if re.match(r"^assert(?![A-Za-z_0-9])", code):
            lines.append(number)
    return lines


# ======================================================================================
# 2. Lexer
# ======================================================================================

class Token:
    __slots__ = ("kind", "value", "line")

    def __init__(self, kind, value, line):
        self.kind = kind      # 'int' | 'str' | 'ident' | 'kw' | 'op' | 'builtin' | 'eof'
        self.value = value
        self.line = line

    def __repr__(self):
        return "Token(%s, %r, line %d)" % (self.kind, self.value, self.line)


class Lexer:
    def __init__(self, path, text):
        self.path = path
        self.text = text

    def fail(self, line, message):
        raise SpecError(self.path, line, message)

    def tokens(self):
        out = []
        for number, raw in enumerate(self.text.splitlines(), 1):
            stripped = raw.strip()
            if not stripped or stripped.startswith(";") or stripped.startswith("//"):
                continue
            out.extend(self._line_tokens(number, raw))
        out.append(Token("eof", None, len(self.text.splitlines()) + 1))
        return out

    def _line_tokens(self, number, raw):
        out = []
        index = 0
        length = len(raw)
        while index < length:
            char = raw[index]
            if char in " \t\r":
                index += 1
                continue
            if char == "/" and raw[index + 1:index + 2] == "/":
                break
            if char == '"':
                value, index = self._string(number, raw, index)
                out.append(Token("str", value, number))
                continue
            if char.isdigit():
                start = index
                while index < length and raw[index].isdigit():
                    index += 1
                if index < length and (raw[index].isalpha() or raw[index] == "_"):
                    self.fail(number, "unsupported numeric literal %r" % raw[start:index + 8])
                out.append(Token("int", int(raw[start:index]), number))
                continue
            if char == "@":
                index += 1
                start = index
                while index < length and (raw[index].isalnum() or raw[index] == "_"):
                    index += 1
                if start == index:
                    self.fail(number, "'@' is not followed by a builtin name")
                out.append(Token("builtin", raw[start:index], number))
                continue
            if char.isalpha() or char == "_":
                start = index
                while index < length and (raw[index].isalnum() or raw[index] == "_"):
                    index += 1
                word = raw[start:index]
                out.append(Token("kw" if word in KEYWORDS else "ident", word, number))
                continue
            if char == ";":
                # A statement terminator. The comment role of ';' is handled above by
                # position; enforce here that the two roles never overlap, so a line
                # that ever puts CODE after a ';' becomes a parse error instead of a
                # silently truncated statement.
                #
                # A trailing '//' comment is not code, and refusing it would make this
                # front end reject a spec the compiler accepts. Measured 2026-09-21
                # against the pinned t27c @ 40003ed: a file whose every statement ends
                # `...; // note` answers {"errors": 0, "warnings": 0, "ok": true} to
                # `typecheck --json` and parses with complete bodies. The tracked corpus
                # carries 0 such lines today, so this allows a form nobody has written
                # yet rather than changing how any current line parses. The '//' is
                # found in raw text, which is safe here precisely because anything other
                # than whitespace-then-'//' after the ';' is refused on the next line.
                #
                # WHAT IS STILL REFUSED, AND THAT IT IS LEGAL. A one-line block --
                # `pub fn one() i32 { return 1; }` -- is refused here, and measured
                # 2026-09-21 the pinned t27c accepts it with {"errors": 0, "warnings":
                # 0, "ok": true} and a complete body. That refusal is deliberate, not an
                # oversight: the positional reading of ';' rests on a measurement (on
                # all 19714 code lines the first statement-terminating ';' is the last
                # non-whitespace character, 0 exceptions), and this is the tripwire that
                # fires if the measurement stops holding. It costs a LOUD red build with
                # an accurate message, never a silent mis-parse, and the corpus carries
                # 0 such lines. Anyone who writes one should split it across lines, or
                # relax this after re-measuring -- not read the refusal as "illegal".
                rest = raw[index + 1:].strip()
                if rest and not rest.startswith("//"):
                    self.fail(number, "unexpected code after ';' -- this gate reads a "
                                      "non-leading ';' as a statement terminator, "
                                      "followed at most by a '//' comment")
                out.append(Token("op", ";", number))
                index += 1
                continue
            for operator in OPERATORS:
                if raw.startswith(operator, index):
                    out.append(Token("op", operator, number))
                    index += len(operator)
                    break
            else:
                self.fail(number, "unexpected character %r" % char)
        return out

    def _string(self, number, raw, index):
        index += 1
        chars = []
        while index < len(raw):
            char = raw[index]
            if char == BACKSLASH:
                nxt = raw[index + 1:index + 2]
                if nxt == "":
                    self.fail(number, "string ends with a dangling escape")
                # The corpus uses exactly one escape: 4 occurrences of \" (measured
                # 2026-09-21). Anything else is refused rather than guessed at.
                if nxt == '"':
                    chars.append('"')
                elif nxt == BACKSLASH:
                    chars.append(BACKSLASH)
                else:
                    self.fail(number, "unsupported string escape %r" % (BACKSLASH + nxt))
                index += 2
                continue
            if char == '"':
                return "".join(chars), index + 1
            chars.append(char)
            index += 1
        self.fail(number, "string literal is not closed before end of line")


# ======================================================================================
# 3. Parser
# ======================================================================================
#
# AST shapes, all plain tuples whose first element is the tag:
#
#   expressions
#     ('int', value, line)                  ('str', value, line)
#     ('bool', value, line)                 ('name', identifier, line)
#     ('call', name, [args], line)          ('cast', type, expr, line)
#     ('index', base, index, line)          ('field', base, field_name, line)
#     ('binop', op, left, right, line)      ('unop', op, operand, line)
#     ('logic', 'and'|'or', left, right, line)
#     ('ifexp', cond, then, other, line)
#     ('structlit', type_name, [(field, expr)], line)
#   statements
#     ('assert', expr, line)                ('return', expr_or_None, line)
#     ('if', cond, then_block, else_block_or_None, line)
#     ('while', cond, continue_stmt_or_None, body, line)
#     ('local', 'var'|'const', name, type_or_None, expr, line)
#     ('assign', name, op, expr, line)
#   declarations
#     ('const', name, type_or_None, expr, line)
#     ('structdef', name, [(field, type)], line, 'const-packed'|'struct')
#     ('fn', name, [(param, type)], return_type, block, line)
#     ('test', name, block, line)           ('invariant', name, assert_stmt, line)
#
# Types: a scalar is its own name as a string ('u8', 'str', 'bool', or a struct name);
# an array is ('array', length, element_type).

class Parser:
    def __init__(self, path, tokens):
        self.path = path
        self.tokens = tokens
        self.pos = 0

    # -- token helpers ----------------------------------------------------------
    def peek(self, offset=0):
        return self.tokens[min(self.pos + offset, len(self.tokens) - 1)]

    def next(self):
        token = self.tokens[self.pos]
        if token.kind != "eof":
            self.pos += 1
        return token

    def at(self, kind, value=None):
        token = self.peek()
        return token.kind == kind and (value is None or token.value == value)

    def accept(self, kind, value=None):
        if self.at(kind, value):
            return self.next()
        return None

    def expect(self, kind, value=None):
        if self.at(kind, value):
            return self.next()
        token = self.peek()
        self.fail(token.line, "expected %s, got %s %r" % (
            value if value is not None else kind, token.kind, token.value))

    def fail(self, line, message):
        raise SpecError(self.path, line, message)

    # -- entry point ------------------------------------------------------------
    def parse_module(self):
        module_name = None
        declarations = []
        while not self.at("eof"):
            token = self.peek()
            if token.kind == "kw" and token.value == "module":
                if module_name is not None:
                    self.fail(token.line, "a second 'module' declaration")
                module_name = self.parse_module_decl()
                continue
            declarations.append(self.parse_declaration())
        if module_name is None:
            self.fail(1, "no 'module' declaration")
        return module_name, declarations

    def parse_module_decl(self):
        self.expect("kw", "module")
        # The module name is the one place a '-' appears outside a string literal
        # (e.g. `module rental-terms;`), so it is read as raw text up to the ';'
        # rather than as an expression.
        parts = []
        while not self.at("op", ";"):
            if self.at("eof"):
                self.fail(self.peek().line, "module declaration is not terminated")
            parts.append(str(self.next().value))
        self.expect("op", ";")
        return "".join(parts)

    # -- declarations -----------------------------------------------------------
    def parse_declaration(self):
        self.accept("kw", "pub")
        token = self.peek()
        if token.kind != "kw":
            self.fail(token.line, "expected a declaration, got %r" % (token.value,))
        if token.value == "const":
            return self.parse_const_decl()
        if token.value == "struct":
            return self.parse_struct_decl()
        if token.value == "fn":
            return self.parse_fn_decl()
        if token.value == "test":
            return self.parse_test_decl()
        if token.value == "invariant":
            return self.parse_invariant_decl()
        self.fail(token.line, "unsupported top-level declaration %r" % token.value)

    def parse_const_decl(self):
        line = self.expect("kw", "const").line
        name = self.expect("ident").value
        declared = None
        if self.accept("op", ":"):
            declared = self.parse_type()
        self.expect("op", "=")
        if self.at("kw", "packed") or self.at("kw", "struct"):
            # `pub const UnitSlot = packed struct { ... };`
            # specs/turbobaby/availability.t27:224, bike_catalog.t27:227, ride_game.t27:198.
            self.accept("kw", "packed")
            self.expect("kw", "struct")
            fields = self.parse_struct_body()
            self.accept("op", ";")
            return ("structdef", name, fields, line, "const-packed")
        expression = self.parse_expr()
        self.expect("op", ";")
        return ("const", name, declared, expression, line)

    def parse_struct_decl(self):
        # `pub struct OptionalRate { ... }` -- specs/turbobaby/pricing_honesty.t27:144.
        line = self.expect("kw", "struct").line
        name = self.expect("ident").value
        fields = self.parse_struct_body()
        self.accept("op", ";")
        return ("structdef", name, fields, line, "struct")

    def parse_struct_body(self):
        self.expect("op", "{")
        fields = []
        while not self.at("op", "}"):
            field = self.expect("ident").value
            self.expect("op", ":")
            fields.append((field, self.parse_type()))
            if not self.accept("op", ","):
                break
        self.expect("op", "}")
        if not fields:
            self.fail(self.peek().line, "struct declares no fields")
        return fields

    def parse_fn_decl(self):
        line = self.expect("kw", "fn").line
        name = self.expect("ident").value
        self.expect("op", "(")
        params = []
        while not self.at("op", ")"):
            param = self.expect("ident").value
            self.expect("op", ":")
            params.append((param, self.parse_type()))
            if not self.accept("op", ","):
                break
        self.expect("op", ")")
        return_type = self.parse_type()
        body = self.parse_block()
        return ("fn", name, params, return_type, body, line)

    def parse_test_decl(self):
        line = self.expect("kw", "test").line
        name = self.expect("ident").value
        return ("test", name, self.parse_block(), line)

    def parse_invariant_decl(self):
        # `invariant name` followed by ONE indented assert line, no braces and no
        # semicolon (e.g. specs/turbobaby/rental_terms.t27's
        # the_measured_ladder_holds_six_rungs). All 836 invariants in the corpus had that
        # shape at f4d84a1, which wrote this comment and cited that one by a line number
        # that had moved by a33e500; re-counted 2026-09-22 over this tree, all 916 do,
        # since this parser reads every one of them and the gate is green.
        # WHAT THE CHECK BELOW ACTUALLY DOES, since it is easy to read as more: it
        # refuses a LEFTOVER token on the assertion's own line -- a stray ';', a second
        # statement -- so the tail of such a line cannot be dropped in silence. It does
        # NOT require the assertion to end on the line it starts on, and it must not:
        # an expression that wraps onto the next line is read correctly (verified
        # 2026-09-21 on a copy of rental_terms.t27 with one invariant's assertion split
        # across two lines -- same value, and flipping the constant on the second line
        # still went red), so refusing it would reject a spec the compiler accepts.
        line = self.expect("kw", "invariant").line
        name = self.expect("ident").value
        assert_line = self.expect("kw", "assert").line
        expression = self.parse_expr()
        if self.peek().line == assert_line and not self.at("eof"):
            token = self.peek()
            self.fail(assert_line, "trailing %r after the invariant's assertion" % (token.value,))
        return ("invariant", name, ("assert", expression, assert_line), line)

    def parse_type(self):
        if self.accept("op", "["):
            length = self.expect("int").value
            self.expect("op", "]")
            element = self.expect("ident").value
            return ("array", length, element)
        token = self.expect("ident")
        return token.value

    # -- statements -------------------------------------------------------------
    def parse_block(self):
        self.expect("op", "{")
        statements = []
        while not self.at("op", "}"):
            if self.at("eof"):
                self.fail(self.peek().line, "block is not closed")
            statements.append(self.parse_statement())
        self.expect("op", "}")
        return statements

    def parse_statement(self):
        token = self.peek()
        if token.kind == "kw":
            if token.value == "assert":
                self.next()
                expression = self.parse_expr()
                self.expect("op", ";")
                return ("assert", expression, token.line)
            if token.value == "return":
                self.next()
                if self.accept("op", ";"):
                    return ("return", None, token.line)
                expression = self.parse_expr()
                self.expect("op", ";")
                return ("return", expression, token.line)
            if token.value == "if":
                return self.parse_if_statement()
            if token.value == "while":
                return self.parse_while_statement()
            if token.value in ("var", "const"):
                self.next()
                name = self.expect("ident").value
                declared = self.parse_type() if self.accept("op", ":") else None
                self.expect("op", "=")
                expression = self.parse_expr()
                self.expect("op", ";")
                return ("local", token.value, name, declared, expression, token.line)
            self.fail(token.line, "unsupported statement keyword %r" % token.value)
        if token.kind == "ident":
            statement = self.parse_assignment()
            self.expect("op", ";")
            return statement
        self.fail(token.line, "unsupported statement starting with %r" % (token.value,))

    def parse_assignment(self):
        token = self.expect("ident")
        operator = self.peek()
        if operator.kind != "op" or operator.value not in ("=", "+=", "-=", "*=", "/=", "%="):
            self.fail(token.line, "expected an assignment after %r" % token.value)
        self.next()
        return ("assign", token.value, operator.value, self.parse_expr(), token.line)

    def parse_if_statement(self):
        line = self.expect("kw", "if").line
        self.expect("op", "(")
        condition = self.parse_expr()
        self.expect("op", ")")
        then_block = self.parse_block()
        else_block = None
        if self.accept("kw", "else"):
            else_block = [self.parse_if_statement()] if self.at("kw", "if") else self.parse_block()
        return ("if", condition, then_block, else_block, line)

    def parse_while_statement(self):
        # Two forms in the corpus: a plain `while (cond) { ... }` with the step inside
        # the body (specs/turbobaby/availability.t27:272) and the Zig-style continue
        # expression `while (i < 14) : (i += 1) { ... }` (the three loops of
        # specs/turbobaby/deposit_tiers.t27's families_on_tier and published_rows_at_amount,
        # cited by name since 2026-09-22 because their line numbers kept moving).
        line = self.expect("kw", "while").line
        self.expect("op", "(")
        condition = self.parse_expr()
        self.expect("op", ")")
        continuation = None
        if self.accept("op", ":"):
            self.expect("op", "(")
            continuation = self.parse_assignment()
            self.expect("op", ")")
        return ("while", condition, continuation, self.parse_block(), line)

    # -- expressions ------------------------------------------------------------
    # Precedence, lowest binding first:
    #   or  <  and  <  comparison  <  additive  <  multiplicative  <  unary  <  postfix
    def parse_expr(self):
        return self.parse_or()

    def parse_or(self):
        left = self.parse_and()
        while self.at("kw", "or"):
            line = self.next().line
            left = ("logic", "or", left, self.parse_and(), line)
        return left

    def parse_and(self):
        left = self.parse_comparison()
        while self.at("kw", "and"):
            line = self.next().line
            left = ("logic", "and", left, self.parse_comparison(), line)
        return left

    def parse_comparison(self):
        left = self.parse_additive()
        while self.peek().kind == "op" and self.peek().value in ("==", "!=", "<", "<=", ">", ">="):
            token = self.next()
            left = ("binop", token.value, left, self.parse_additive(), token.line)
        return left

    def parse_additive(self):
        left = self.parse_multiplicative()
        while self.peek().kind == "op" and self.peek().value in ("+", "-"):
            token = self.next()
            left = ("binop", token.value, left, self.parse_multiplicative(), token.line)
        return left

    def parse_multiplicative(self):
        left = self.parse_unary()
        while self.peek().kind == "op" and self.peek().value in ("*", "/", "%"):
            token = self.next()
            left = ("binop", token.value, left, self.parse_unary(), token.line)
        return left

    def parse_unary(self):
        token = self.peek()
        if token.kind == "op" and token.value in ("!", "-"):
            self.next()
            return ("unop", token.value, self.parse_unary(), token.line)
        return self.parse_postfix()

    def parse_postfix(self):
        node = self.parse_primary()
        while True:
            if self.at("op", "["):
                line = self.next().line
                index = self.parse_expr()
                self.expect("op", "]")
                node = ("index", node, index, line)
                continue
            if self.at("op", "."):
                line = self.next().line
                node = ("field", node, self.expect("ident").value, line)
                continue
            return node

    def parse_primary(self):
        token = self.peek()
        if token.kind == "int":
            self.next()
            return ("int", token.value, token.line)
        if token.kind == "str":
            self.next()
            return ("str", token.value, token.line)
        if token.kind == "kw" and token.value in ("true", "false"):
            self.next()
            return ("bool", token.value == "true", token.line)
        if token.kind == "kw" and token.value == "if":
            # `const n = if (used > MAX) MAX else used;` -- the one if-EXPRESSION in the
            # corpus, specs/turbobaby/availability.t27:269.
            self.next()
            self.expect("op", "(")
            condition = self.parse_expr()
            self.expect("op", ")")
            then_value = self.parse_expr()
            self.expect("kw", "else")
            return ("ifexp", condition, then_value, self.parse_expr(), token.line)
        if token.kind == "builtin":
            self.next()
            if token.value != "as":
                self.fail(token.line, "unsupported builtin @%s" % token.value)
            self.expect("op", "(")
            target = self.parse_type()
            self.expect("op", ",")
            inner = self.parse_expr()
            self.expect("op", ")")
            return ("cast", target, inner, token.line)
        if token.kind == "op" and token.value == "(":
            self.next()
            inner = self.parse_expr()
            self.expect("op", ")")
            return inner
        if token.kind == "op" and token.value == "[":
            self.next()
            items = []
            while not self.at("op", "]"):
                items.append(self.parse_expr())
                if not self.accept("op", ","):
                    break
            self.expect("op", "]")
            return ("array", items, token.line)
        if token.kind == "ident":
            self.next()
            if self.at("op", "("):
                self.next()
                args = []
                while not self.at("op", ")"):
                    args.append(self.parse_expr())
                    if not self.accept("op", ","):
                        break
                self.expect("op", ")")
                return ("call", token.value, args, token.line)
            if self.at("op", "{"):
                # `OptionalRate{ .tag = RATE_PRESENT, ... }`
                # specs/turbobaby/pricing_honesty.t27:199. Unambiguous against a block
                # because every `if` and `while` condition in this grammar is
                # parenthesised, so a '{' after an expression is never a block opener.
                self.next()
                fields = []
                while not self.at("op", "}"):
                    self.expect("op", ".")
                    field = self.expect("ident").value
                    self.expect("op", "=")
                    fields.append((field, self.parse_expr()))
                    if not self.accept("op", ","):
                        break
                self.expect("op", "}")
                return ("structlit", token.value, fields, token.line)
            return ("name", token.value, token.line)
        self.fail(token.line, "unexpected %s %r in an expression" % (token.kind, token.value))


# ======================================================================================
# 4. Values and the evaluator
# ======================================================================================

class StructValue:
    __slots__ = ("type_name", "fields")

    def __init__(self, type_name, fields):
        self.type_name = type_name
        self.fields = fields


def render(node):
    """Reconstruct an expression's source text, for failure reports."""
    tag = node[0]
    if tag == "int":
        return str(node[1])
    if tag == "str":
        return '"%s"' % node[1]
    if tag == "bool":
        return "true" if node[1] else "false"
    if tag == "name":
        return node[1]
    if tag == "call":
        return "%s(%s)" % (node[1], ", ".join(render(a) for a in node[2]))
    if tag == "cast":
        return "@as(%s, %s)" % (render_type(node[1]), render(node[2]))
    if tag == "index":
        return "%s[%s]" % (render(node[1]), render(node[2]))
    if tag == "field":
        return "%s.%s" % (render(node[1]), node[2])
    if tag == "binop":
        return "%s %s %s" % (render(node[2]), node[1], render(node[3]))
    if tag == "unop":
        return "%s%s" % (node[1], render(node[2]))
    if tag == "logic":
        return "%s %s %s" % (render(node[2]), node[1], render(node[3]))
    if tag == "ifexp":
        return "if (%s) %s else %s" % (render(node[1]), render(node[2]), render(node[3]))
    if tag == "structlit":
        return "%s{ %s }" % (node[1], ", ".join(".%s = %s" % (f, render(e)) for f, e in node[2]))
    if tag == "array":
        return "[%s]" % ", ".join(render(item) for item in node[1])
    return "<%s>" % tag


def render_type(declared):
    if isinstance(declared, tuple):
        return "[%d]%s" % (declared[1], declared[2])
    return declared


# A failure report a reader cannot act on is half a gate -- but one that prints a
# 22-element array of English prose is no better. Values are shown whole up to these
# sizes and elided after, with the full length stated so nothing looks complete when it
# is not.
SHOW_MAX_STRING = 120
SHOW_MAX_ITEMS = 8


def show(value):
    """One-line rendering of a runtime value for a failure report."""
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        if len(value) > SHOW_MAX_STRING:
            return '"%s..." (%d chars)' % (value[:SHOW_MAX_STRING], len(value))
        return '"%s"' % value
    if isinstance(value, list):
        shown = ", ".join(show(item) for item in value[:SHOW_MAX_ITEMS])
        if len(value) > SHOW_MAX_ITEMS:
            return "[%s, ... ] (%d items)" % (shown, len(value))
        return "[%s]" % shown
    if isinstance(value, StructValue):
        return "%s{ %s }" % (value.type_name,
                             ", ".join(".%s = %s" % (k, show(v)) for k, v in value.fields.items()))
    return str(value)


class _Return(Exception):
    def __init__(self, value):
        super().__init__("return")
        self.value = value


class Failure:
    """One failed assertion, with everything a reader needs to act on it."""

    def __init__(self, path, line, source, detail):
        self.path = path
        self.line = line
        self.source = source
        self.detail = detail

    def report(self):
        return "  %s:%d\n      %s\n      %s" % (rel(self.path), self.line, self.source, self.detail)


class Evaluator:
    """Executes one spec.

    DECISIONS TAKEN HERE, each of them a semantics choice rather than an accident:

    * Integers are Python ints, so no intermediate result is ever silently truncated.
      Every DECLARED width is enforced where a value enters a typed slot -- a const, a
      var, a function parameter, a function return, a struct field, an array element,
      or an @as cast. Out of range is a reported FAILURE, never a wrap.
    * '/' is integer division truncating toward zero, and '%' is the remainder that
      matches it (C semantics, not Python's floor semantics).
      WHAT IS AND IS NOT MEASURED HERE. That '/' discards the fraction rather than
      rounding IS measured: specs/turbobaby/rental_terms.t27's audit_daily_thb (cited by
      name since 2026-09-22; the line numbers written at f4d84a1 had moved by a33e500),
      whose asserted value requires (449*7500*2 + 10000) / 20000 == 337,
      i.e. 337.25 discarded rather than rounded to 337.25 -> 337.5 -> 338. Which way it
      discards for a NEGATIVE operand is NOT measured and cannot be, because the corpus
      never divides one: instrumenting Evaluator.arith over all 43 specs on 2026-09-21
      counted 0 divisions and 0 remainders with a negative left or right operand, and
      337.25 truncates and floors to the same 337. Truncation is chosen because it is
      what C and Rust do, which is what the generators emit; it is a decision taken on
      the target languages, not a fact read off this corpus.
    * 'and' and 'or' short-circuit; '!' is boolean negation and refuses a non-bool.
    * '.len' is the DECLARED array length. It is not defined on a str here: the corpus
      never asks for it (measured 0 uses on a non-array), and guessing between bytes
      and characters would be inventing a number.
    * An unknown function, an undefined identifier, an out-of-range index, a division
      by zero, a type mismatch, too deep a recursion or too long a loop is a reported
      FAILURE naming file, line and expression -- never an exception that kills the run.
    """

    def __init__(self, path, module_name, declarations):
        self.path = path
        self.module_name = module_name
        self.consts = {}
        self.structs = {}
        self.functions = {}
        self.tests = []
        self.invariants = []
        self.const_nodes = {}
        self.const_cache = {}
        self.resolving = []
        self.depth = 0
        for declaration in declarations:
            tag = declaration[0]
            if tag == "const":
                if declaration[1] in self.const_nodes:
                    raise SpecError(path, declaration[4], "duplicate const %r" % declaration[1])
                self.const_nodes[declaration[1]] = declaration
            elif tag == "structdef":
                # Guarded for the same reason as const and fn above, and it was not:
                # a dict assignment lets a second declaration of the same name replace
                # the first silently, after which every struct literal is validated
                # against the wrong field set. Measured 2026-09-21: a copy of
                # specs/turbobaby/availability.t27 carrying a second one-field
                # `pub const UnitSlot = packed struct` ran green, because no literal of
                # that struct exists in that file to notice the missing field.
                if declaration[1] in self.structs:
                    raise SpecError(path, declaration[3],
                                    "duplicate struct %r" % declaration[1])
                self.structs[declaration[1]] = declaration[2]
            elif tag == "fn":
                if declaration[1] in self.functions:
                    raise SpecError(path, declaration[5], "duplicate fn %r" % declaration[1])
                self.functions[declaration[1]] = declaration
            elif tag == "test":
                self.tests.append(declaration)
            elif tag == "invariant":
                self.invariants.append(declaration)

    # -- errors ------------------------------------------------------------------
    def bad(self, line, message):
        raise SpecError(self.path, line, message)

    # -- type enforcement --------------------------------------------------------
    def coerce(self, declared, value, line, what):
        """Check `value` against `declared` and return it. Bounds are real."""
        if declared is None:
            return value
        if isinstance(declared, tuple) and declared[0] == "array":
            if not isinstance(value, list):
                self.bad(line, "%s: expected an array, got %s" % (what, show(value)))
            if len(value) != declared[1]:
                self.bad(line, "%s: declared [%d]%s but holds %d element(s)"
                         % (what, declared[1], declared[2], len(value)))
            return [self.coerce(declared[2], item, line, "%s[%d]" % (what, i))
                    for i, item in enumerate(value)]
        if declared in INT_RANGES:
            if isinstance(value, bool) or not isinstance(value, int):
                self.bad(line, "%s: expected %s, got %s" % (what, declared, show(value)))
            low, high = INT_RANGES[declared]
            if not low <= value <= high:
                self.bad(line, "%s: %d is outside %s (%d..%d) -- the declared width is a "
                               "bound, so this is a defect, not a wrap"
                         % (what, value, declared, low, high))
            return value
        if declared == "bool":
            if not isinstance(value, bool):
                self.bad(line, "%s: expected bool, got %s" % (what, show(value)))
            return value
        if declared == "str":
            if not isinstance(value, str):
                self.bad(line, "%s: expected str, got %s" % (what, show(value)))
            return value
        if declared in self.structs:
            if not isinstance(value, StructValue) or value.type_name != declared:
                self.bad(line, "%s: expected %s, got %s" % (what, declared, show(value)))
            return value
        self.bad(line, "%s: unknown type %r" % (what, render_type(declared)))

    # -- constants ---------------------------------------------------------------
    def const(self, name, line):
        if name in self.const_cache:
            return self.const_cache[name]
        node = self.const_nodes.get(name)
        if node is None:
            self.bad(line, "undefined identifier %r" % name)
        if name in self.resolving:
            self.bad(line, "const %r is defined in terms of itself (%s)"
                     % (name, " -> ".join(self.resolving + [name])))
        self.resolving.append(name)
        try:
            value = self.eval(node[3], {})
            value = self.coerce(node[2], value, node[4], "const %s" % name)
        finally:
            self.resolving.pop()
        self.const_cache[name] = value
        return value

    def warm_all_constants(self):
        """Force every top-level const, so a broken one is a failure even if no test
        happens to read it. Returns the number evaluated."""
        for name in self.const_nodes:
            self.const(name, self.const_nodes[name][4])
        return len(self.const_nodes)

    # -- statements --------------------------------------------------------------
    def exec_block(self, statements, env, on_assert):
        for statement in statements:
            self.exec_statement(statement, env, on_assert)

    def exec_statement(self, statement, env, on_assert):
        tag = statement[0]
        if tag == "assert":
            on_assert(statement, env)
            return
        if tag == "return":
            raise _Return(None if statement[1] is None else self.eval(statement[1], env))
        if tag == "local":
            _, kind, name, declared, expression, line = statement
            value = self.coerce(declared, self.eval(expression, env), line, "%s %s" % (kind, name))
            # A local's slot carries its declared type and whether it may be rebound, so
            # both are enforced at the assignment rather than trusted.
            env[name] = (value, declared, kind)
            return
        if tag == "assign":
            _, name, operator, expression, line = statement
            if name not in env:
                self.bad(line, "assignment to undeclared local %r" % name)
            current, declared, kind = env[name]
            if kind == "const":
                self.bad(line, "%r is a const local and cannot be assigned to" % name)
            value = self.eval(expression, env)
            if operator != "=":
                # Compound assignment is integer arithmetic. Checked here because
                # self.arith would otherwise concatenate two strings, or add two
                # booleans as 0 and 1, without anybody noticing.
                for operand, what in ((current, name), (value, render(expression))):
                    if isinstance(operand, bool) or not isinstance(operand, int):
                        self.bad(line, "%s is %s; '%s' is integer arithmetic"
                                 % (what, show(operand), operator))
                value = self.arith(operator[0], current, value, line)
            env[name] = (self.coerce(declared, value, line, name), declared, kind)
            return
        if tag == "if":
            _, condition, then_block, else_block, line = statement
            if self.as_bool(self.eval(condition, env), line, render(condition)):
                self.exec_block(then_block, env, on_assert)
            elif else_block is not None:
                self.exec_block(else_block, env, on_assert)
            return
        if tag == "while":
            _, condition, continuation, body, line = statement
            iterations = 0
            while self.as_bool(self.eval(condition, env), line, render(condition)):
                iterations += 1
                if iterations > MAX_LOOP_ITERATIONS:
                    self.bad(line, "loop ran past %d iterations" % MAX_LOOP_ITERATIONS)
                self.exec_block(body, env, on_assert)
                if continuation is not None:
                    self.exec_statement(continuation, env, on_assert)
            return
        self.bad(statement[-1], "unsupported statement %r" % tag)

    # -- expressions -------------------------------------------------------------
    def eval(self, node, env):
        tag = node[0]
        if tag in ("int", "str", "bool"):
            return node[1]
        if tag == "name":
            if node[1] in env:
                return env[node[1]][0]
            return self.const(node[1], node[2])
        if tag == "array":
            return [self.eval(item, env) for item in node[1]]
        if tag == "call":
            return self.call(node, env)
        if tag == "cast":
            value = self.eval(node[2], env)
            return self.coerce(node[1], value, node[3], "@as(%s, %s)" % (render_type(node[1]), render(node[2])))
        if tag == "index":
            base = self.eval(node[1], env)
            index = self.eval(node[2], env)
            if not isinstance(base, list):
                self.bad(node[3], "%s is not an array" % render(node[1]))
            if isinstance(index, bool) or not isinstance(index, int):
                self.bad(node[3], "index %s is not an integer" % render(node[2]))
            if not 0 <= index < len(base):
                self.bad(node[3], "index %d is outside %s, which holds %d element(s)"
                         % (index, render(node[1]), len(base)))
            return base[index]
        if tag == "field":
            base = self.eval(node[1], env)
            field = node[2]
            if isinstance(base, list):
                if field != "len":
                    self.bad(node[3], "an array has no field %r" % field)
                return len(base)
            if isinstance(base, StructValue):
                if field not in base.fields:
                    self.bad(node[3], "%s has no field %r" % (base.type_name, field))
                return base.fields[field]
            if isinstance(base, str) and field == "len":
                self.bad(node[3], "'.len' on a str is not defined by this evaluator "
                                  "(bytes or characters is unstated, and guessing would "
                                  "invent a number)")
            self.bad(node[3], "%s is not a struct or array" % render(node[1]))
        if tag == "unop":
            value = self.eval(node[2], env)
            if node[1] == "!":
                return not self.as_bool(value, node[3], render(node[2]))
            if isinstance(value, bool) or not isinstance(value, int):
                self.bad(node[3], "unary '-' needs an integer, got %s" % show(value))
            return -value
        if tag == "logic":
            left = self.as_bool(self.eval(node[2], env), node[4], render(node[2]))
            if node[1] == "and":
                if not left:
                    return False
                return self.as_bool(self.eval(node[3], env), node[4], render(node[3]))
            if left:
                return True
            return self.as_bool(self.eval(node[3], env), node[4], render(node[3]))
        if tag == "ifexp":
            if self.as_bool(self.eval(node[1], env), node[4], render(node[1])):
                return self.eval(node[2], env)
            return self.eval(node[3], env)
        if tag == "binop":
            return self.binop(node, env)
        if tag == "structlit":
            return self.struct_literal(node, env)
        self.bad(node[-1], "unsupported expression %r" % tag)

    def as_bool(self, value, line, source):
        if not isinstance(value, bool):
            self.bad(line, "%s is %s, not a boolean" % (source, show(value)))
        return value

    def binop(self, node, env):
        _, operator, left_node, right_node, line = node
        left = self.eval(left_node, env)
        right = self.eval(right_node, env)
        if operator in ("==", "!="):
            same = (isinstance(left, bool) == isinstance(right, bool)
                    and isinstance(left, str) == isinstance(right, str)
                    and isinstance(left, int) == isinstance(right, int))
            if not same or isinstance(left, (list, StructValue)):
                self.bad(line, "cannot compare %s with %s (%s %s %s)"
                         % (show(left), show(right), render(left_node), operator, render(right_node)))
            return (left == right) if operator == "==" else (left != right)
        if operator in ("<", "<=", ">", ">="):
            for value, source in ((left, left_node), (right, right_node)):
                if isinstance(value, bool) or not isinstance(value, int):
                    self.bad(line, "%s is %s; '%s' compares integers"
                             % (render(source), show(value), operator))
            if operator == "<":
                return left < right
            if operator == "<=":
                return left <= right
            if operator == ">":
                return left > right
            return left >= right
        for value, source in ((left, left_node), (right, right_node)):
            if isinstance(value, bool) or not isinstance(value, int):
                self.bad(line, "%s is %s; '%s' is integer arithmetic"
                         % (render(source), show(value), operator))
        return self.arith(operator, left, right, line)

    def arith(self, operator, left, right, line):
        if operator == "+":
            return left + right
        if operator == "-":
            return left - right
        if operator == "*":
            return left * right
        if operator in ("/", "%"):
            if right == 0:
                self.bad(line, "division by zero")
            # Truncate toward zero, not Python's floor -- see the class docstring.
            quotient = abs(left) // abs(right)
            if (left < 0) != (right < 0):
                quotient = -quotient
            if operator == "/":
                return quotient
            return left - quotient * right
        self.bad(line, "unsupported operator %r" % operator)

    def struct_literal(self, node, env):
        _, type_name, fields, line = node
        declared = self.structs.get(type_name)
        if declared is None:
            self.bad(line, "unknown struct %r" % type_name)
        types = dict(declared)
        values = {}
        for field, expression in fields:
            if field not in types:
                self.bad(line, "%s has no field %r" % (type_name, field))
            if field in values:
                self.bad(line, "%s sets %r twice" % (type_name, field))
            values[field] = self.coerce(types[field], self.eval(expression, env), line,
                                        "%s.%s" % (type_name, field))
        missing = [name for name, _ in declared if name not in values]
        if missing:
            self.bad(line, "%s literal leaves %s unset" % (type_name, ", ".join(missing)))
        return StructValue(type_name, {name: values[name] for name, _ in declared})

    def call(self, node, env):
        _, name, args, line = node
        function = self.functions.get(name)
        if function is None:
            self.bad(line, "call to unknown function %r" % name)
        _, _, params, return_type, body, declared_line = function
        if len(args) != len(params):
            self.bad(line, "%s takes %d argument(s), given %d" % (name, len(params), len(args)))
        frame = {}
        for (param, declared), argument in zip(params, args):
            frame[param] = (self.coerce(declared, self.eval(argument, env), line,
                                        "%s(%s)" % (name, param)), declared, "var")
        if self.depth >= MAX_CALL_DEPTH:
            self.bad(line, "call depth passed %d frames at %s" % (MAX_CALL_DEPTH, name))
        self.depth += 1
        try:
            self.exec_block(body, frame, self._no_asserts)
        except _Return as returned:
            if returned.value is None:
                self.bad(line, "%s returned no value but declares %s"
                         % (name, render_type(return_type)))
            return self.coerce(return_type, returned.value, declared_line, "%s -> %s" % (name, render_type(return_type)))
        finally:
            self.depth -= 1
        self.bad(line, "%s reached the end of its body without returning" % name)

    def _no_asserts(self, statement, env):
        # Measured 2026-09-21: all 7888 assert statements in the corpus sit inside a
        # `test` or an `invariant` block and none inside a function body. If that ever
        # changes, this refuses rather than executing an assertion the coverage check
        # is not counting.
        self.bad(statement[2], "assert inside a function body is not supported")


# ======================================================================================
# 5. Running one spec
# ======================================================================================

class SpecResult:
    def __init__(self, path):
        self.path = path
        self.module_name = None
        self.executed = 0
        self.passed = 0
        self.failures = []
        self.errors = []
        self.sites = set()
        self.scanned = []
        self.declarations = []
        self.spec_id = None
        self.constants = 0
        self.tests = 0
        self.invariants = 0

    @property
    def missed(self):
        """Assert lines the scanner found that the evaluator never reached."""
        return sorted(set(self.scanned) - self.sites)


def explain(evaluator, node, env):
    """Say why an assertion came out false, in terms a reader can act on."""
    tag = node[0]
    try:
        if tag == "logic" and node[1] == "and":
            for side in (node[2], node[3]):
                if evaluator.eval(side, env) is False:
                    return explain(evaluator, side, env)
        if tag == "logic" and node[1] == "or":
            return "both sides false: (%s) and (%s)" % (
                explain(evaluator, node[2], env), explain(evaluator, node[3], env))
        if tag == "binop" and node[1] in ("==", "!=", "<", "<=", ">", ">="):
            left = evaluator.eval(node[2], env)
            right = evaluator.eval(node[3], env)
            return "%s = %s   |   %s = %s" % (
                render(node[2]), show(left), render(node[3]), show(right))
        if tag == "unop" and node[1] == "!":
            return "%s = %s, so the negation is false" % (render(node[2]),
                                                          show(evaluator.eval(node[2], env)))
        if tag == "call":
            return "%s = false" % render(node)
    except SpecError as error:
        return "could not be explained: %s" % error.message
    return "%s = false" % render(node)


def static_assert_count(statements):
    """How many assert statements a block CONTAINS, whether or not they are reached.

    Counted from the parse tree, not from execution, because the two answer different
    questions: execution coverage is checked against the text scanner further down, and
    this one exists to catch a block that has nothing to reach.
    """
    total = 0
    for statement in statements:
        tag = statement[0]
        if tag == "assert":
            total += 1
        elif tag == "if":
            total += static_assert_count(statement[2])
            if statement[3] is not None:
                total += static_assert_count(statement[3])
        elif tag == "while":
            total += static_assert_count(statement[3])
    return total


def run_spec(path, text):
    """Parse and execute one spec. Never raises for spec content; collects instead."""
    result = SpecResult(path)
    result.scanned = scan_assert_lines(path, text)
    try:
        module_name, declarations = Parser(path, Lexer(path, text).tokens()).parse_module()
    except SpecError as error:
        result.errors.append(str(error))
        return result
    result.module_name = module_name
    result.declarations = declarations
    try:
        evaluator = Evaluator(path, module_name, declarations)
        result.constants = evaluator.warm_all_constants()
        result.spec_id = evaluator.const_cache.get("ID")
    except SpecError as error:
        result.errors.append(str(error))
        return result

    source_lines = text.splitlines()

    def make_recorder(block_label):
        def record(statement, env):
            _, expression, line = statement
            result.executed += 1
            result.sites.add(line)
            source = source_lines[line - 1].strip() if line - 1 < len(source_lines) else render(expression)
            try:
                value = evaluator.eval(expression, env)
            except SpecError as error:
                result.failures.append(Failure(path, line, source,
                                               "%s [in %s] could not be evaluated: %s"
                                               % (render(expression), block_label, error.message)))
                return
            if not isinstance(value, bool):
                result.failures.append(Failure(path, line, source,
                                               "[in %s] the assertion is %s, not a boolean"
                                               % (block_label, show(value))))
                return
            if value:
                result.passed += 1
                return
            result.failures.append(Failure(path, line, source,
                                           "[in %s] %s" % (block_label, explain(evaluator, expression, env))))
        return record

    for declaration in declarations:
        if declaration[0] == "test":
            result.tests += 1
            label = "test %s" % declaration[1]
            # A test block that contains no assert is the D16 shape exactly, and it is
            # the one gutting that the WHOLE PAIR of gates is blind to. Measured
            # 2026-09-21 against the pinned t27c @ 40003ed: emptying a test body leaves
            # its AST inventory byte for byte identical -- 3 declarations, one TestBlock
            # with children: [], typecheck {"errors": 0, "warnings": 0, "ok": true} --
            # so scripts/verify_t27_specs.py's min_declarations and min_checks floors
            # cannot see it either, because they count blocks and t27c discards bodies.
            # Neither can the coverage equality below: an assert that is not there is
            # not scanned, so scanned and executed still agree. Before this check, a
            # copy of specs/turbobaby/rental_terms.t27 with one six-assert test body
            # deleted ran green at 101 of 107. Corpus cost, measured the same day: 0 of
            # the 1013 test blocks assert nothing, so this refuses a shape that does not
            # exist yet rather than grandfathering one that does.
            if static_assert_count(declaration[2]) == 0:
                result.errors.append(
                    "%s:%d: %s asserts nothing -- a block that checks nothing reads as a "
                    "passing test from the outside (D16)" % (rel(path), declaration[3], label))
            try:
                evaluator.exec_block(declaration[2], {}, make_recorder(label))
            except _Return:
                result.errors.append("%s:%d: 'return' outside a function, in %s"
                                     % (rel(path), declaration[3], label))
            except SpecError as error:
                result.errors.append("%s [%s aborted here; later assertions in it did not run]"
                                     % (error, label))
        elif declaration[0] == "invariant":
            result.invariants += 1
            label = "invariant %s" % declaration[1]
            try:
                make_recorder(label)(declaration[2], {})
            except SpecError as error:
                result.errors.append("%s [%s]" % (error, label))
    return result


# ======================================================================================
# 6. Cross-check against the pinned external compiler
# ======================================================================================

# The pinned compiler does NOT hand back test or invariant bodies, but it DOES hand
# back function bodies -- so the two front ends overlap on 858 functions and 2002
# statements, and that overlap is checked node for node. Both sides are reduced to the
# canonical strings below; anything that differs is a place where one of the two readers
# is wrong about what the corpus says.
#
# MEASURED DISAGREEMENTS, pinned the way scripts/verify_t27_specs.py pins its floors.
# The gate fails on any disagreement NOT listed here, AND on any listed disagreement
# that has gone away -- a stale allowance is the same defect as no allowance at all.
KNOWN_FRONTEND_DISAGREEMENTS = {
    # Measured 2026-09-21 against t27c @ 40003ed. Both functions use the Zig-style
    # `while (cond) : (step) { ... }` continue expression (one loop in families_on_tier,
    # two in published_rows_at_amount). t27c's parser stops at the ':' and silently
    # discards the REST OF THE FUNCTION -- the loop, any later local, and the `return` --
    # with no error and no warning: `typecheck --json` still answers
    # {"errors": 0, "warnings": 0, "ok": true}.
    # It then generates `uint8_t families_on_tier(uint8_t tier_index) { uint8_t found =
    # 0; uint8_t i = 0; }` in C and the same shape in Rust: a value-returning function
    # whose body contains no return statement and no tail expression. This evaluator
    # reads the loop and runs it, which is why deposit_tiers.t27's 257 assertions pass
    # here while the generated code for these two functions returns nothing.
    ("specs/turbobaby/deposit_tiers.t27", "families_on_tier"):
        "t27c drops the `while (i < 14) : (i += 1)` loop and the `return found;` after it",
    ("specs/turbobaby/deposit_tiers.t27", "published_rows_at_amount"):
        "t27c drops both `: (i += 1)` loops, the `var j` between them, and the return",

    # Measured 2026-09-21, a SECOND and unrelated class. t27c truncates a function body
    # at the first `;`-style comment inside it. Minimal reproduction, all three shapes,
    # against the same pinned build:
    #   `; note` first in a body            -> the body parses to []
    #   `; note` between two statements     -> everything after it is dropped
    #   `; note` first inside a nested block-> that block parses to {}
    # A `//` comment in any of those positions parses correctly, so the trigger is the
    # `;` form specifically. Corpus reach is exactly these two functions -- they are the
    # only two whose body span contains a `;` comment line.
    # Consequence, measured: `t27c gen-c specs/turbobaby/commerce.t27` emits
    # `uint8_t commerce_checkout_decision(...) { /* TODO: implement */ }` -- all nine
    # checkout gates gone -- and `gen-rust specs/turbobaby/deposit_tiers.t27` emits a
    # `refund_decision` that returns nothing on every path past the first gate. Both
    # files still typecheck as {"errors": 0, "warnings": 0, "ok": true}, so
    # scripts/verify_t27_specs.py is green on them.
    ("specs/turbobaby/commerce.t27", "commerce_checkout_decision"):
        "t27c truncates the body at the leading `;` comment, losing all nine gates",
    ("specs/turbobaby/deposit_tiers.t27", "refund_decision"):
        "t27c truncates at the `;` comments, losing the passport branch and everything after",
}


def canonical_mine(node):
    """Reduce this parser's AST to the canonical string the comparison uses."""
    tag = node[0]
    if tag == "int":
        return "L(%d)" % node[1]
    if tag == "bool":
        return "L(%s)" % ("true" if node[1] else "false")
    if tag == "str":
        return "S(%s)" % node[1]
    if tag == "name":
        return "I(%s)" % node[1]
    if tag == "call":
        return "C(%s;%s)" % (node[1], ",".join(canonical_mine(a) for a in node[2]))
    if tag == "cast":
        # t27c models @as as an ordinary call whose first argument is the type name.
        return "C(@as;I(%s),%s)" % (render_type(node[1]), canonical_mine(node[2]))
    if tag == "index":
        return "X(%s,%s)" % (canonical_mine(node[1]), canonical_mine(node[2]))
    if tag == "field":
        return "F(%s;%s)" % (node[2], canonical_mine(node[1]))
    if tag in ("binop", "logic"):
        return "B(%s;%s,%s)" % (node[1], canonical_mine(node[2]), canonical_mine(node[3]))
    if tag == "unop":
        return "U(%s;%s)" % (node[1], canonical_mine(node[2]))
    if tag == "ifexp":
        return "IE(%s,%s,%s)" % (canonical_mine(node[1]), canonical_mine(node[2]),
                                 canonical_mine(node[3]))
    if tag == "structlit":
        return "SL(%s;%s)" % (node[1], ",".join("F(%s;%s)" % (f, canonical_mine(e))
                                                for f, e in node[2]))
    if tag == "array":
        return "A(%s)" % ",".join(canonical_mine(item) for item in node[1])
    if tag == "return":
        return "R(%s)" % ("" if node[1] is None else canonical_mine(node[1]))
    if tag == "local":
        return "LOC(%s:%s=%s)" % (node[2], render_type(node[3]) if node[3] else "",
                                  canonical_mine(node[4]))
    if tag == "assign":
        return "AS(%s;I(%s),%s)" % (node[2], node[1], canonical_mine(node[3]))
    if tag == "if":
        tail = "{%s}" % canonical_block_mine(node[3]) if node[3] is not None else ""
        return "IF(%s;{%s}%s)" % (canonical_mine(node[1]), canonical_block_mine(node[2]), tail)
    if tag == "while":
        step = "" if node[2] is None else canonical_mine(node[2])
        return "WH(%s;%s;{%s})" % (canonical_mine(node[1]), step, canonical_block_mine(node[3]))
    if tag == "assert":
        return "ASSERT(%s)" % canonical_mine(node[1])
    return "?%s" % tag


def canonical_block_mine(statements):
    return ";".join(canonical_mine(s) for s in statements)


def canonical_theirs(node):
    """Reduce a t27c AST node to the same canonical string."""
    kind = node.get("kind")
    children = node.get("children") or []
    if kind == "ExprLiteral":
        if node.get("extra_kind") == "string":
            return "S(%s)" % node.get("value", "")
        return "L(%s)" % node.get("value", "")
    if kind == "ExprIdentifier":
        return "I(%s)" % node.get("name", "")
    if kind == "ExprCall":
        return "C(%s;%s)" % (node.get("name", ""), ",".join(canonical_theirs(c) for c in children))
    if kind == "ExprIndex":
        return "X(%s,%s)" % (canonical_theirs(children[0]), canonical_theirs(children[1]))
    if kind == "ExprFieldAccess":
        return "F(%s;%s)" % (node.get("name", ""), canonical_theirs(children[0]))
    if kind == "ExprBinary":
        return "B(%s;%s,%s)" % (node.get("extra_op", ""), canonical_theirs(children[0]),
                                canonical_theirs(children[1]))
    if kind == "ExprUnary":
        return "U(%s;%s)" % (node.get("extra_op", ""), canonical_theirs(children[0]))
    if kind == "ExprIf":
        return "IE(%s,%s,%s)" % tuple(canonical_theirs(c) for c in children[:3])
    if kind == "ExprStructLit":
        return "SL(%s;%s)" % (node.get("name", ""), ",".join(canonical_theirs(c) for c in children))
    if kind == "ExprReturn":
        return "R(%s)" % (canonical_theirs(children[0]) if children else "")
    if kind == "StmtLocal":
        return "LOC(%s:%s=%s)" % (node.get("name", ""), node.get("extra_type", ""),
                                  canonical_theirs(children[0]) if children else "")
    if kind == "StmtAssign":
        # t27c leaves extra_op empty for a plain '=' and fills it only for '+=' and
        # friends; normalise so the two readers spell the same operator.
        return "AS(%s;%s,%s)" % (node.get("extra_op") or "=", canonical_theirs(children[0]),
                                 canonical_theirs(children[1]))
    if kind == "StmtIf":
        blocks = [c for c in children[1:] if c.get("kind") == "Module"]
        tail = "{%s}" % canonical_block_theirs(blocks[1]) if len(blocks) > 1 else ""
        return "IF(%s;{%s}%s)" % (canonical_theirs(children[0]),
                                  canonical_block_theirs(blocks[0]) if blocks else "", tail)
    if kind == "StmtWhile":
        blocks = [c for c in children[1:] if c.get("kind") == "Module"]
        return "WH(%s;;{%s})" % (canonical_theirs(children[0]),
                                 canonical_block_theirs(blocks[0]) if blocks else "")
    if kind == "Module":
        return "{%s}" % canonical_block_theirs(node)
    return "?%s" % kind


def canonical_block_theirs(module_node):
    return ";".join(canonical_theirs(c) for c in (module_node.get("children") or []))


def compare_function_bodies(path, declarations, compiler_children):
    """Node-for-node comparison of every function body. Returns (compared, [problems])."""
    theirs = {}
    for child in compiler_children:
        if child.get("kind") == "FnDecl":
            theirs[child.get("name")] = child
    problems = []
    compared = 0
    seen_known = set()
    for declaration in declarations:
        if declaration[0] != "fn":
            continue
        name = declaration[1]
        their_fn = theirs.get(name)
        if their_fn is None:
            problems.append("%s: t27c reports no FnDecl for %s" % (rel(path), name))
            continue
        compared += 1
        mine = canonical_block_mine(declaration[4])
        yours = canonical_block_theirs(their_fn)
        my_params = [[p, render_type(t)] for p, t in declaration[2]]
        their_params = [list(p) for p in (their_fn.get("params") or [])]
        if my_params != their_params:
            problems.append("%s: %s parameters disagree -- t27c %s, this parser %s"
                            % (rel(path), name, their_params, my_params))
        if render_type(declaration[3]) != their_fn.get("extra_return_type", ""):
            problems.append("%s: %s return type disagrees -- t27c %r, this parser %r"
                            % (rel(path), name, their_fn.get("extra_return_type"),
                               render_type(declaration[3])))
        if mine == yours:
            continue
        key = (rel(path), name)
        if key in KNOWN_FRONTEND_DISAGREEMENTS:
            seen_known.add(key)
            continue
        problems.append("%s: %s body disagrees between the two front ends\n"
                        "        t27c reads: %s\n"
                        "        this reads: %s" % (rel(path), name, yours[:400], mine[:400]))
    # A pin that has stopped describing reality is the same defect as no pin at all, so
    # it is reported rather than left to rot. Both ways it can go stale are covered:
    # the disagreement was fixed, or the function it names is gone.
    for key, why in KNOWN_FRONTEND_DISAGREEMENTS.items():
        spec_name, fn_name = key
        if spec_name != rel(path) or key in seen_known:
            continue
        reason = ("no longer disagrees with t27c" if fn_name in theirs
                  else "is not a function t27c reports in this spec")
        problems.append("%s: pinned front-end disagreement %r %s (pinned as: %s) -- "
                        "re-measure and remove the KNOWN_FRONTEND_DISAGREEMENTS entry"
                        % (spec_name, fn_name, reason, why))
    return compared, problems


def resolve_compiler(cli_path, require):
    """Same escape surface as scripts/verify_t27_specs.py: T27C, or --t27c, and a
    --require-compiler flag that turns a missing compiler into a failure. CI is always
    required."""
    configured = cli_path or os.environ.get("T27C")
    if configured:
        compiler = Path(configured).expanduser().resolve()
        if not compiler.is_file():
            return None, "T27C is not a file: %s" % compiler
        return compiler, None
    if require or os.environ.get("CI", "").lower() in {"1", "true", "yes"}:
        return None, "T27C is required in CI; set it to the pinned external t27c executable"
    return None, None


def crosscheck(compiler, path, declarations, evaluator_consts):
    """Compare what the two front ends see in one file.

    The compiler is pinned at gHashTag/t27 40003ed1379c8a417e13e45843de35088b88f8c0.
    Three measured properties of that build are allowed for by NAME rather than
    reported as drift -- and each is re-asserted, so that a compiler bump which fixes
    one of them turns this gate red instead of passing unnoticed:

      * A `pub const Name = packed struct { ... };` is split into TWO top-level nodes:
        a `ConstDecl` carrying the name, whose single child is an `ExprIdentifier`
        (the struct body never becomes a value), and a separate `StructDecl` whose
        `name` is the EMPTY STRING and whose children are bare `ExprIdentifier`s.
        Measured 2026-09-21 on specs/turbobaby/availability.t27:224 (nodes 48 and 49
        of the parse), and the same on bike_catalog.t27:227 and ride_game.t27:198.
        Two consequences worth stating: the struct's name and its field names never
        reach the AST at all, and the declaration floors in
        scripts/verify_t27_specs.py count one such declaration twice.
      * A `pub struct Name { ... }` (pricing_honesty.t27:144, 180) keeps its name and
        produces no ConstDecl.
      * TestBlock and InvariantBlock arrive with `children: []`, which is the whole
        reason this gate exists; only their NAMES can be compared.

    Returns (compared_count, [problems]).
    """
    process = subprocess.run([str(compiler), "parse", str(path), "--json"],
                             cwd=str(REPO), text=True, capture_output=True, check=False)
    if process.returncode != 0:
        return 0, ["%s: t27c parse exited %d: %s"
                   % (rel(path), process.returncode, (process.stderr or process.stdout).strip()[:300])]
    try:
        ast = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        return 0, ["%s: t27c parse --json was not JSON: %s" % (rel(path), error)]

    problems = []
    children = ast.get("children") or []
    theirs = {"ConstDecl": set(), "FnDecl": set(), "TestBlock": set(), "InvariantBlock": set()}
    their_structs = set()
    their_struct_count = 0
    for child in children:
        kind = child.get("kind")
        if kind in theirs:
            theirs[kind].add(child.get("name"))
        elif kind == "StructDecl":
            their_struct_count += 1
            if child.get("name"):
                their_structs.add(child["name"])

    mine = {"ConstDecl": set(), "FnDecl": set(), "TestBlock": set(), "InvariantBlock": set()}
    my_structs = set()
    my_packed_consts = set()
    for declaration in declarations:
        tag = declaration[0]
        if tag == "const":
            mine["ConstDecl"].add(declaration[1])
        elif tag == "fn":
            mine["FnDecl"].add(declaration[1])
        elif tag == "test":
            mine["TestBlock"].add(declaration[1])
        elif tag == "invariant":
            mine["InvariantBlock"].add(declaration[1])
        elif tag == "structdef":
            my_structs.add(declaration[1])
            if declaration[4] == "const-packed":
                my_packed_consts.add(declaration[1])

    # The pinned build's split, re-asserted rather than assumed: each
    # `pub const Name = packed struct {...};` must show up as a ConstDecl named Name
    # AND as one anonymous StructDecl. If a future compiler stops doing that, the two
    # checks below go red and this comment is what needs revisiting.
    missing_phantom = sorted(my_packed_consts - theirs["ConstDecl"])
    if missing_phantom:
        problems.append("%s: t27c no longer emits the phantom ConstDecl for packed-struct "
                        "const(s) %s -- the pinned-compiler quirk this cross-check allows "
                        "for has changed; re-measure it" % (rel(path), missing_phantom))
    expected_anonymous = len(my_packed_consts)
    anonymous = their_struct_count - len(their_structs)
    if anonymous != expected_anonymous:
        problems.append("%s: t27c emitted %d anonymous StructDecl(s); %d packed-struct "
                        "const(s) were declared" % (rel(path), anonymous, expected_anonymous))

    compared = 0
    for kind in ("ConstDecl", "FnDecl", "TestBlock", "InvariantBlock"):
        expected = mine[kind] | (my_packed_consts if kind == "ConstDecl" else set())
        compared += len(theirs[kind] | expected)
        only_theirs = sorted(theirs[kind] - expected)
        only_mine = sorted(expected - theirs[kind])
        if only_theirs:
            problems.append("%s: t27c reports %s %s that this parser did not find"
                            % (rel(path), kind, only_theirs[:8]))
        if only_mine:
            problems.append("%s: this parser found %s %s that t27c does not report"
                            % (rel(path), kind, only_mine[:8]))
    compared += len(my_structs)
    if their_struct_count != len(my_structs):
        problems.append("%s: t27c reports %d struct declaration(s), this parser found %d %s"
                        % (rel(path), their_struct_count, len(my_structs), sorted(my_structs)))
    if their_structs - my_structs:
        problems.append("%s: t27c names struct(s) %s that this parser did not find"
                        % (rel(path), sorted(their_structs - my_structs)))

    their_id = None
    for child in children:
        if child.get("kind") == "ConstDecl" and child.get("name") == "ID":
            for literal in child.get("children", []):
                if literal.get("kind") == "ExprLiteral" and isinstance(literal.get("value"), str):
                    their_id = literal["value"]
    my_id = evaluator_consts.get("ID")
    if their_id is None:
        problems.append("%s: t27c reports no ID string literal" % rel(path))
    elif my_id != their_id:
        problems.append("%s: ID disagrees -- t27c %r, this parser %r" % (rel(path), their_id, my_id))
    else:
        compared += 1

    # The overlap that matters most: t27c keeps function bodies, so every statement in
    # every function is read twice and the two readings must match.
    body_compared, body_problems = compare_function_bodies(path, declarations, children)
    compared += body_compared
    problems.extend(body_problems)
    return compared, problems


# ======================================================================================
# 7. CLI
# ======================================================================================

def discover_specs():
    return sorted(SPECS_ROOT.rglob("*.t27"))


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("-v", "--verbose", action="store_true",
                        help="print a line per spec and a final total")
    parser.add_argument("--file", help="run one spec while debugging; corpus floors are not applied")
    parser.add_argument("--t27c", help="path to the pinned external t27c executable")
    parser.add_argument("--require-compiler", action="store_true",
                        help="fail instead of skipping the cross-check when no compiler is configured")
    parser.add_argument("--no-crosscheck", action="store_true",
                        help="execute assertions only; do not compare against t27c")
    return parser.parse_args()


def main():
    args = parse_args()
    problems = []

    if args.file:
        target = Path(args.file)
        if not target.is_absolute():
            target = REPO / args.file
        if not target.is_file():
            print("execute_t27_assertions: FAIL - no such spec: %s" % args.file, file=sys.stderr)
            return 1
        specs = [target]
    else:
        specs = discover_specs()
        if len(specs) < MIN_SPEC_FILES:
            print("execute_t27_assertions: FAIL - discovery found %d spec(s) under %s; the "
                  "floor is %d (D16: a gate whose input can reach zero must pin a floor)"
                  % (len(specs), rel(SPECS_ROOT), MIN_SPEC_FILES), file=sys.stderr)
            return 1

    results = []
    for spec in specs:
        try:
            text = spec.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            # UnicodeDecodeError is not an OSError, so a spec that is not UTF-8 -- a
            # UTF-16 export, say -- used to escape this handler and end the run in a
            # raw traceback. The exit code was still 1, but a traceback is not the
            # single 'name: FAIL - reason' line the sibling gates print.
            problems.append("%s: could not be read: %s" % (rel(spec), error))
            continue
        try:
            results.append(run_spec(spec, text))
        except SpecError as error:
            failed = SpecResult(spec)
            failed.errors.append(str(error))
            results.append(failed)

    total_scanned = sum(len(result.scanned) for result in results)
    total_executed = sum(result.executed for result in results)
    total_passed = sum(result.passed for result in results)
    total_failed = sum(len(result.failures) for result in results)

    if not args.file and total_scanned < MIN_ASSERT_LINES:
        problems.append("the assert scanner found %d assert line(s) in %d spec(s); the floor is "
                        "%d. An empty or shrunken scan is a blind gate, not a green one (D16)."
                        % (total_scanned, len(results), MIN_ASSERT_LINES))

    for result in results:
        for error in result.errors:
            problems.append(error)
        if result.missed:
            problems.append("%s: %d assert line(s) were scanned but never executed: %s"
                            % (rel(result.path), len(result.missed),
                               ", ".join(str(line) for line in result.missed[:12])))
        extra = sorted(result.sites - set(result.scanned))
        if extra:
            problems.append("%s: executed assertions at line(s) %s that the text scanner did not "
                            "find -- the two readers disagree about where the asserts are"
                            % (rel(result.path), extra[:12]))

    compared = 0
    known_disagreements = 0
    if not args.no_crosscheck:
        compiler, message = resolve_compiler(args.t27c, args.require_compiler)
        if compiler is None and message:
            problems.append(message)
        elif compiler is None:
            print("execute_t27_assertions: cross-check SKIP - set T27C to compare against the "
                  "pinned external compiler", file=sys.stderr)
        else:
            checked = set()
            for result in results:
                if result.module_name is None:
                    continue
                checked.add(rel(result.path))
                count, found = crosscheck(compiler, result.path, result.declarations,
                                          {"ID": result.spec_id})
                compared += count
                problems.extend(found)
            # Printed on every run, green or not, and only for the specs this run
            # actually compared. A known defect that stops being visible is a defect
            # nobody fixes.
            for (spec_path, fn_name), why in sorted(KNOWN_FRONTEND_DISAGREEMENTS.items()):
                if spec_path not in checked:
                    continue
                known_disagreements += 1
                print("execute_t27_assertions: PINNED front-end disagreement - %s %s: %s"
                      % (spec_path, fn_name, why), file=sys.stderr)
            # THE THIRD WAY A PIN GOES STALE, and the one the per-file guard inside
            # compare_function_bodies() structurally cannot see: that guard runs once per
            # spec the run READ, so a pinned spec that is no longer in the corpus takes
            # its pins out of enforcement without a word. Measured 2026-09-21: a copy of
            # the corpus with commerce.t27 and deposit_tiers.t27 removed ran GREEN with
            # all four pins silently unenforced and the summary reporting none. In
            # --file mode only one spec is read on purpose, so the check would fire on
            # the other 42 and is skipped there.
            if not args.file:
                for spec_path in sorted({key[0] for key in KNOWN_FRONTEND_DISAGREEMENTS}):
                    if spec_path not in checked:
                        problems.append(
                            "%s carries pinned front-end disagreement(s) but was not "
                            "cross-checked in this run -- the spec is missing, renamed or "
                            "did not parse, so the pin is unenforced; re-measure and update "
                            "KNOWN_FRONTEND_DISAGREEMENTS" % spec_path)

    if args.verbose:
        for result in results:
            print("execute_t27_assertions: %-44s asserts=%-5d passed=%-5d failed=%-3d "
                  "tests=%-3d invariants=%-3d consts=%d"
                  % (rel(result.path), result.executed, result.passed,
                     len(result.failures), result.tests, result.invariants, result.constants))

    if total_failed:
        print("execute_t27_assertions: FAIL - %d assertion(s) are false or unevaluable:"
              % total_failed, file=sys.stderr)
        for result in results:
            for failure in result.failures:
                print(failure.report(), file=sys.stderr)
    if problems:
        print("execute_t27_assertions: FAIL - %d structural problem(s):" % len(problems),
              file=sys.stderr)
        for problem in problems:
            print("  - %s" % problem, file=sys.stderr)

    summary = ("%d spec(s), %d assert line(s) scanned, %d executed, %d passed, %d failed"
               % (len(results), total_scanned, total_executed, total_passed, total_failed))
    if compared:
        summary += "; %d declaration name(s) and function bodies agreed with t27c" % compared
        if known_disagreements:
            summary += "; %d pinned front-end disagreement(s)" % known_disagreements
    if total_failed or problems:
        print("execute_t27_assertions: %s" % summary, file=sys.stderr)
        return 1
    print("execute_t27_assertions: OK - %s" % summary)
    if args.verbose:
        print("execute_t27_assertions: floors - files>=%d, assert lines>=%d, and every scanned "
              "assert line executed" % (MIN_SPEC_FILES, MIN_ASSERT_LINES))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
