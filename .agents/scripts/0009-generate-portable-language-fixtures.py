#!/usr/bin/env python3
#DESCRIPTION: Generate portable JSON, YAML-like, and ws fixtures from canonical tests/language/*.lisp fixtures.
#USAGE: .agents/scripts/0009-generate-portable-language-fixtures.py [--check]

from __future__ import annotations

import argparse
import json
import math
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable


@dataclass(frozen=True)
class Atom:
    text: str


@dataclass(frozen=True)
class String:
    value: str


@dataclass(frozen=True)
class Form:
    items: tuple["Datum", ...]


Datum = Atom | String | Form
Renderer = Callable[[list[Datum]], str]

READER_PREFIXES = {"`", ",", ",@"}
YAML_READER_ALIASES = {
    "`": "quasiquote",
    ",": "unquote",
    ",@": "unquote-splicing",
}
WS_INLINE_FIRST_FORM_HEADS = {"fn", "defn", "let"}
INT_RE = re.compile(r"-?\d+\Z")
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1
WS_LINE_LIMIT = 88


class Parser:
    def __init__(self, source: str) -> None:
        self.source = source
        self.index = 0

    def parse_module(self) -> list[Datum]:
        forms: list[Datum] = []
        self.skip_layout()
        while not self.done():
            forms.append(self.parse_datum())
            self.skip_layout()
        return forms

    def parse_datum(self) -> Datum:
        self.skip_layout()
        if self.done():
            raise ValueError("unexpected end of Lisp input")
        char = self.source[self.index]
        if char == "(":
            return self.parse_form()
        if char == '"':
            return self.parse_string()
        if char == "`":
            self.index += 1
            return Form((Atom("`"), self.parse_datum()))
        if char == ",":
            if self.source.startswith(",@", self.index):
                self.index += 2
                return Form((Atom(",@"), self.parse_datum()))
            self.index += 1
            return Form((Atom(","), self.parse_datum()))
        if char == ")":
            raise ValueError("unexpected `)`")
        return self.parse_atom()

    def parse_form(self) -> Form:
        self.index += 1
        items: list[Datum] = []
        while True:
            self.skip_layout()
            if self.done():
                raise ValueError("unterminated Lisp form")
            if self.source[self.index] == ")":
                self.index += 1
                return Form(tuple(items))
            items.append(self.parse_datum())

    def parse_string(self) -> String:
        start = self.index
        self.index += 1
        escaped = False
        while not self.done():
            char = self.source[self.index]
            self.index += 1
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                return String(json.loads(self.source[start : self.index]))
        raise ValueError("unterminated string literal")

    def parse_atom(self) -> Atom:
        start = self.index
        while not self.done():
            char = self.source[self.index]
            if char.isspace() or char in '();"`,':
                break
            self.index += 1
        if self.index == start:
            raise ValueError(f"unexpected character {self.source[self.index]!r}")
        return Atom(self.source[start : self.index])

    def skip_layout(self) -> None:
        while not self.done():
            while not self.done() and self.source[self.index].isspace():
                self.index += 1
            if not self.done() and self.source[self.index] == ";":
                while not self.done() and self.source[self.index] != "\n":
                    self.index += 1
                continue
            return

    def done(self) -> bool:
        return self.index >= len(self.source)


def render_json_module(forms: list[Datum]) -> str:
    return json.dumps([json_value(form) for form in forms], ensure_ascii=False, indent=2) + "\n"


def json_value(datum: Datum) -> Any:
    if isinstance(datum, Atom):
        return atom_json_value(datum.text)
    if isinstance(datum, String):
        return ["str", datum.value]
    return [json_value(item) for item in datum.items]


def atom_json_value(text: str) -> Any:
    if text == "null":
        return None
    if text == "true":
        return True
    if text == "false":
        return False
    if INT_RE.fullmatch(text):
        value = int(text)
        if I64_MIN <= value <= I64_MAX:
            return value
        return text
    if looks_like_float(text):
        try:
            value = float(text)
        except ValueError:
            return text
        if math.isfinite(value):
            return value
    return text


def render_yaml_module(forms: list[Datum]) -> str:
    return render_yaml_node(Form(tuple(forms))) + "\n"


def render_yaml_node(datum: Datum) -> str:
    inline = render_yaml_inline(datum)
    if len(inline) <= WS_LINE_LIMIT:
        return inline
    if not isinstance(datum, Form):
        return inline

    lines = ["["]
    for item in datum.items:
        rendered = suffix_last_line(render_yaml_node(item), ",")
        lines.append(indent_block(rendered, 2))
    lines.append("]")
    return "\n".join(lines)


def render_yaml_inline(datum: Datum) -> str:
    if isinstance(datum, Atom):
        return render_yaml_atom(datum.text)
    if isinstance(datum, String):
        return quote_string(datum.value)
    return "[" + ", ".join(render_yaml_inline(item) for item in datum.items) + "]"


def render_yaml_atom(text: str) -> str:
    if text in YAML_READER_ALIASES:
        return YAML_READER_ALIASES[text]
    if is_yaml_plain_symbol(text):
        return text
    raise ValueError(f"cannot render symbol {text!r} in YAML-like syntax")


def is_yaml_plain_symbol(text: str) -> bool:
    if not text:
        return False
    if text.startswith(("'", '"')):
        return False
    return not any(char.isspace() or char in ",[]{}#" for char in text)


def suffix_last_line(text: str, suffix: str) -> str:
    lines = text.splitlines()
    lines[-1] += suffix
    return "\n".join(lines)


def indent_block(text: str, spaces: int) -> str:
    prefix = " " * spaces
    return "\n".join(prefix + line if line else line for line in text.splitlines())


def render_ws_module(forms: list[Datum], *, prefer_continuations: bool = False) -> str:
    return (
        "\n\n".join(
            render_ws_layout(form, prefer_continuations=prefer_continuations) for form in forms
        )
        + "\n"
    )


def render_ws_layout(
    datum: Datum, indent: int = 0, *, prefer_continuations: bool = False
) -> str:
    prefix = " " * indent
    if isinstance(datum, (Atom, String)):
        return prefix + render_ws_inline(datum)
    if is_reader_form(datum):
        return prefix + render_ws_inline(datum)
    if len(datum.items) <= 1:
        return prefix + render_ws_sexpr(datum)

    head = datum.items[0]
    if not isinstance(head, Atom):
        lines = [prefix + render_ws_inline(head)]
        append_ws_child_lines(
            lines, datum.items, 1, indent, prefer_continuations=prefer_continuations
        )
        return "\n".join(lines)
    if head.text == "obj":
        obj_layout = render_ws_obj_layout(
            datum, indent, prefer_continuations=prefer_continuations
        )
        if obj_layout is not None:
            return obj_layout

    inline = [render_ws_inline(head)]
    inline_form_count = 0
    index = 1
    broke_on_line_limit = False
    while index < len(datum.items):
        item = datum.items[index]
        if not can_ws_inline(item):
            break
        if is_ws_form_argument(item):
            if not should_inline_ws_form_argument(head.text, inline_form_count):
                break
            inline_form_count += 1
        rendered = render_ws_inline(item)
        candidate = " ".join([*inline, rendered])
        if len(prefix) + len(candidate) > WS_LINE_LIMIT:
            broke_on_line_limit = True
            break
        inline.append(rendered)
        index += 1

    if broke_on_line_limit and all(is_ws_scalar(item) for item in datum.items[1:]):
        inline = [render_ws_inline(head)]
        index = 1

    lines = [prefix + " ".join(inline)]
    append_ws_child_lines(
        lines, datum.items, index, indent, prefer_continuations=prefer_continuations
    )
    return "\n".join(lines)


def render_ws_obj_layout(
    datum: Form, indent: int, *, prefer_continuations: bool
) -> str | None:
    if (len(datum.items) - 1) % 2 != 0:
        return None

    prefix = " " * indent
    child_prefix = " " * (indent + 2)
    lines = [prefix + "obj"]
    can_extend_head = True
    args = datum.items[1:]

    for index in range(0, len(args), 2):
        key = args[index]
        value = args[index + 1]
        pair = render_ws_obj_pair_inline(key, value)
        if pair is not None:
            if can_extend_head:
                candidate = prefix + "obj " + pair
                if len(candidate) <= WS_LINE_LIMIT:
                    lines[0] = candidate
                    can_extend_head = False
                    continue
            candidate = child_prefix + "... " + pair
            if len(candidate) <= WS_LINE_LIMIT:
                lines.append(candidate)
                can_extend_head = False
                continue

        append_ws_obj_item(lines, key, indent, can_extend_head, prefer_continuations)
        if can_extend_head and len(lines) == 1:
            can_extend_head = False
        lines.append(
            render_ws_layout(value, indent + 2, prefer_continuations=prefer_continuations)
        )
        can_extend_head = False

    return "\n".join(lines)


def render_ws_obj_pair_inline(key: Datum, value: Datum) -> str | None:
    if not can_ws_inline(key) or not can_ws_inline(value):
        return None
    return render_ws_inline(key) + " " + render_ws_inline(value)


def append_ws_obj_item(
    lines: list[str],
    item: Datum,
    indent: int,
    can_extend_head: bool,
    prefer_continuations: bool,
) -> None:
    prefix = " " * indent
    if can_extend_head and can_ws_inline(item):
        candidate = prefix + "obj " + render_ws_inline(item)
        if len(candidate) <= WS_LINE_LIMIT:
            lines[0] = candidate
            return
    lines.append(
        render_ws_layout(item, indent + 2, prefer_continuations=prefer_continuations)
    )


def append_ws_child_lines(
    lines: list[str],
    items: tuple[Datum, ...],
    index: int,
    indent: int,
    *,
    prefer_continuations: bool,
) -> None:
    while index < len(items):
        item = items[index]
        if is_ws_scalar(item):
            run: list[Datum] = []
            while index < len(items) and is_ws_scalar(items[index]):
                run.append(items[index])
                index += 1
            lines.extend(render_ws_scalar_run(run, indent + 2, prefer_continuations))
            continue
        lines.append(
            render_ws_layout(item, indent + 2, prefer_continuations=prefer_continuations)
        )
        index += 1


def render_ws_scalar_run(
    items: list[Datum], indent: int, prefer_continuations: bool
) -> list[str]:
    prefix = " " * indent
    rendered = [render_ws_inline(item) for item in items]
    if not prefer_continuations:
        lines: list[str] = []
        continuation: list[str] = []
        for token in rendered:
            if token.startswith("..."):
                continuation.append(token)
                continue
            if continuation:
                lines.append(prefix + "... " + " ".join(continuation))
                continuation.clear()
            lines.append(prefix + token)
        if continuation:
            lines.append(prefix + "... " + " ".join(continuation))
        return lines

    if len(items) == 1 and not rendered[0].startswith("..."):
        return [prefix + rendered[0]]
    return [prefix + "... " + " ".join(rendered)]


def is_ws_scalar(datum: Datum) -> bool:
    return isinstance(datum, (Atom, String)) or is_reader_form(datum)


def is_ws_form_argument(datum: Datum) -> bool:
    return isinstance(datum, Form) and not is_reader_form(datum)


def should_inline_ws_form_argument(parent_head: str, inline_form_count: int) -> bool:
    return parent_head in WS_INLINE_FIRST_FORM_HEADS and inline_form_count == 0


def can_ws_inline(datum: Datum) -> bool:
    if is_ws_scalar(datum):
        return True
    if not isinstance(datum, Form):
        return False
    if is_reader_form(datum):
        return True
    return node_count(datum) <= 8 and form_depth(datum) <= 3


def node_count(datum: Datum) -> int:
    if isinstance(datum, (Atom, String)):
        return 1
    return 1 + sum(node_count(item) for item in datum.items)


def form_depth(datum: Datum) -> int:
    if isinstance(datum, (Atom, String)):
        return 0
    return 1 + max((form_depth(item) for item in datum.items), default=0)


def render_ws_inline(datum: Datum) -> str:
    if isinstance(datum, Atom):
        return datum.text
    if isinstance(datum, String):
        return quote_string(datum.value)
    if is_reader_form(datum):
        head = datum.items[0]
        assert isinstance(head, Atom)
        return head.text + render_reader_arg(datum.items[1])
    return render_ws_sexpr(datum)


def render_reader_arg(datum: Datum) -> str:
    if isinstance(datum, Form):
        return render_ws_sexpr(datum)
    return render_ws_inline(datum)


def render_ws_sexpr(datum: Datum) -> str:
    if isinstance(datum, (Atom, String)):
        return render_ws_inline(datum)
    if is_reader_form(datum):
        return render_ws_inline(datum)
    return "(" + " ".join(render_ws_sexpr(item) for item in datum.items) + ")"


def is_reader_form(datum: Datum) -> bool:
    return (
        isinstance(datum, Form)
        and len(datum.items) == 2
        and isinstance(datum.items[0], Atom)
        and datum.items[0].text in READER_PREFIXES
    )


def quote_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def looks_like_float(text: str) -> bool:
    return any(char in text for char in ".eE") and all(
        char.isdigit() or char in "-+.eE" for char in text
    )


def canonical_fixture_paths(root: Path) -> list[Path]:
    return sorted(
        path
        for directory in ("language", "ui")
        for path in (root / "tests" / directory).glob("*.lisp")
    )


def read_forms(path: Path) -> list[Datum]:
    return Parser(path.read_text(encoding="utf-8")).parse_module()


def generated_targets(root: Path, source: Path, forms: list[Datum]) -> dict[Path, str]:
    stem = source.stem
    prefer_ws_continuations = stem == "ws-layout-continuation"
    suite = source.parent.name
    generated = root / "tests" / f"generated-{suite}"
    return {
        generated / "json" / f"{stem}.json": render_json_module(forms),
        generated / "yaml" / f"{stem}.yaml": render_yaml_module(forms),
        generated / "ws" / f"{stem}.ws": render_ws_module(
            forms, prefer_continuations=prefer_ws_continuations
        ),
    }


def stale_generated_paths(root: Path, expected: set[Path]) -> list[Path]:
    paths: list[Path] = []
    for generated_name in ("generated-language", "generated-ui"):
        generated_root = root / "tests" / generated_name
        if not generated_root.exists():
            continue
        for suffix in ("*.json", "*.yaml", "*.yml", "*.ws"):
            paths.extend(generated_root.glob(f"*/*{suffix[1:]}"))
    return sorted(path for path in paths if path not in expected)


def relative(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[2]
    changed = False
    expected: set[Path] = set()

    for source in canonical_fixture_paths(root):
        forms = read_forms(source)
        for target, output in generated_targets(root, source, forms).items():
            expected.add(target)
            if target.exists() and target.read_text(encoding="utf-8") == output:
                continue
            changed = True
            if args.check:
                print(f"would update {relative(target, root)}")
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(output, encoding="utf-8")
            print(f"updated {relative(target, root)}")

    for target in stale_generated_paths(root, expected):
        changed = True
        if args.check:
            print(f"would remove {relative(target, root)}")
            continue
        target.unlink()
        print(f"removed {relative(target, root)}")

    return 1 if args.check and changed else 0


if __name__ == "__main__":
    raise SystemExit(main())
