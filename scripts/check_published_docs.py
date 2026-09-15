#!/usr/bin/env python3
"""Reject missing local files and fragments in the generated documentation."""

from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


class DocumentLinks(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.fragments: set[str] = set()
        self.links: list[str] = []

    def handle_starttag(
        self, tag: str, attributes: list[tuple[str, str | None]]
    ) -> None:
        values = dict(attributes)
        fragment = values.get("id")
        if fragment is not None:
            self.fragments.add(fragment)
        if tag == "a" and values.get("href") is not None:
            self.links.append(values["href"])


def parse_document(path: Path) -> DocumentLinks:
    parser = DocumentLinks()
    parser.feed(path.read_text(encoding="utf-8"))
    return parser


def local_target(site: Path, source: Path, link_path: str) -> Path:
    decoded = unquote(link_path)
    if decoded.startswith("/"):
        return site / decoded.removeprefix("/")
    return source.parent / decoded


def validate(site: Path) -> list[str]:
    documents = {
        path.resolve(): parse_document(path) for path in site.rglob("*.html")
    }
    failures: list[str] = []

    for source, document in documents.items():
        for href in document.links:
            link = urlsplit(href)
            if link.scheme or link.netloc:
                continue

            target = local_target(site.resolve(), source, link.path)
            if not link.path:
                target = source
            elif link.path.endswith("/"):
                target = target / "index.html"

            target = target.resolve()
            if not target.exists():
                failures.append(f"{source.relative_to(site.resolve())}: {href}")
                continue

            source_parts = source.relative_to(site.resolve()).parts
            target_parts = target.relative_to(site.resolve()).parts
            is_guide_link = source_parts[0] == "guide" and target_parts[0] == "guide"
            if link.fragment and target.suffix == ".html" and is_guide_link:
                target_document = documents.get(target)
                if target_document is None:
                    target_document = parse_document(target)
                    documents[target] = target_document
                if link.fragment not in target_document.fragments:
                    failures.append(
                        f"{source.relative_to(site.resolve())}: {href} (missing fragment)"
                    )

    return failures


def main() -> int:
    arguments = argparse.ArgumentParser()
    arguments.add_argument("site", type=Path)
    site = arguments.parse_args().site

    failures = validate(site)
    if failures:
        print("Broken generated-documentation links:")
        for failure in failures:
            print(f"- {failure}")
        return 1

    print(f"Validated local links under {site}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
