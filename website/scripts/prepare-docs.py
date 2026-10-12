"""Prepare an explicit selection of repository Markdown for the static website."""

from __future__ import annotations

import json
from pathlib import Path
import re
import shutil
import tempfile
from urllib.parse import quote, unquote, urlsplit


ROOT = Path(__file__).resolve().parents[2]
LINK = re.compile(r"(\[[^\]]*\]\()(<[^>\n]+>|[^\s)\n]+)([^)\n]*\))")
REFERENCE = re.compile(
    r"^([ \t]{0,3}\[[^\]\n]+\]:[ \t]*)(<[^>\n]+>|\S+)(.*)$", re.MULTILINE
)
PROSE_TOKEN = re.compile(
    r"(?P<code>(?P<ticks>`+).*?(?P=ticks))"
    r"|(?P<link>\[[^\]]*\]\((?:<[^>\n]+>|[^\s)\n]+)[^)\n]*\))"
    r"|(?P<reference>^[ \t]{0,3}\[[^\]\n]+\]:[ \t]*(?:<[^>\n]+>|\S+)[^\n]*$)",
    re.DOTALL | re.MULTILINE,
)
HEADING = re.compile(r"^# (.+)\n", re.MULTILINE)
SLUG = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*\Z")


def publication(root: Path) -> tuple[list[dict], dict[Path, str]]:
    groups = json.loads((root / "website/docs.config.json").read_text())["groups"]
    routes: dict[Path, str] = {}
    destinations: set[str] = set()
    group_slugs: set[str] = set()
    for group in groups:
        group_slug = group["slug"]
        if group_slug in group_slugs or (group_slug and not SLUG.fullmatch(group_slug)):
            raise ValueError(f"Invalid or repeated group slug: {group_slug!r}")
        group_slugs.add(group_slug)
        for page in group["pages"]:
            if not SLUG.fullmatch(page["slug"]):
                raise ValueError(f"Invalid page slug: {page['slug']!r}")
            source = (root / page["source"]).resolve()
            if not source.is_relative_to(root.resolve()) or not source.is_file():
                raise ValueError(
                    f"Missing or out-of-repository source: {page['source']}"
                )
            if source.suffix != ".md" or not page["title"] or not page["description"]:
                raise ValueError(f"Invalid Markdown page: {page['source']}")
            relative = "/".join(filter(None, [group_slug, page["slug"]]))
            route = (
                relative.removesuffix("index") if page["slug"] == "index" else relative
            )
            url = "/docs/" + (route.rstrip("/") + "/" if route else "")
            if source in routes or url in destinations:
                raise ValueError(f"Repeated source or destination: {page['source']}")
            destinations.add(url)
            routes[source] = url
    return groups, routes


def source_url(root: Path, target: Path) -> str:
    relative = target.relative_to(root.resolve()).as_posix()
    repository = "airlock"
    if relative == "airlock-machine" or relative.startswith("airlock-machine/"):
        repository = "machine"
        relative = relative.removeprefix("airlock-machine").lstrip("/")
    kind = "tree" if target.is_dir() or not relative else "blob"
    return f"https://github.com/airlock-org/{repository}/{kind}/main/{quote(relative)}"


def rewrite_target(
    root: Path, source: Path, target: str, routes: dict[Path, str]
) -> str:
    angled = target.startswith("<") and target.endswith(">")
    value = target[1:-1] if angled else target
    url = urlsplit(value)
    if url.scheme or url.netloc or not url.path or value.startswith("/"):
        return target
    path = (source.parent / unquote(url.path)).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError(
            f"Broken repository link in {source.relative_to(root)}: {value}"
        )
    component = root.resolve() / "airlock-machine"
    unavailable_component = (
        path.is_relative_to(component) and not (component / ".git").exists()
    )
    if not path.exists() and not unavailable_component:
        raise ValueError(
            f"Broken repository link in {source.relative_to(root)}: {value}"
        )
    rewritten = routes.get(path) or source_url(root, path)
    if url.query:
        rewritten += "?" + url.query
    if url.fragment:
        rewritten += "#" + url.fragment
    return f"<{rewritten}>" if angled else rewritten


def rewrite_markdown(
    root: Path, source: Path, text: str, routes: dict[Path, str]
) -> str:
    """Rewrite prose links while preserving fenced and inline code literally."""
    output = []
    prose = []
    fence: str | None = None
    fence_length = 0

    def rewrite_prose(chunk: str) -> str:
        def rewrite_token(token: re.Match) -> str:
            if token["code"]:
                return token[0]
            match = (
                LINK.fullmatch(token[0])
                if token["link"]
                else REFERENCE.fullmatch(token[0])
            )
            assert match
            return match[1] + rewrite_target(root, source, match[2], routes) + match[3]

        return PROSE_TOKEN.sub(rewrite_token, chunk)

    def flush_prose() -> None:
        output.append(rewrite_prose("".join(prose)))
        prose.clear()

    for line in text.splitlines(keepends=True):
        marker = re.match(r"^ {0,3}(`{3,}|~{3,})(.*)$", line)
        if marker:
            run, suffix = marker.groups()
            if fence is None:
                flush_prose()
                fence, fence_length = run[0], len(run)
            elif run[0] == fence and len(run) >= fence_length and not suffix.strip():
                fence = None
            output.append(line)
            continue
        if fence:
            output.append(line)
            continue
        prose.append(line)
    flush_prose()
    return "".join(output)


def prepare(root: Path) -> int:
    root = root.resolve()
    groups, routes = publication(root)
    pages: dict[str, str] = {}
    metadata: dict[str, dict] = {}
    root_order = []
    for group in groups:
        slug = group["slug"]
        if slug:
            root_order.append(slug)
        else:
            root_order.extend(page["slug"] for page in group["pages"])
        metadata[slug] = {
            "title": group["title"],
            "pages": [page["slug"] for page in group["pages"]],
        }
        for page in group["pages"]:
            source = (root / page["source"]).resolve()
            text = source.read_text()
            text = HEADING.sub("", text, count=1).lstrip("\n")
            text = rewrite_markdown(root, source, text, routes)
            header = (
                "---\n"
                + "\n".join(
                    f"{key}: {json.dumps(page[key])}"
                    for key in ("title", "description")
                )
                + "\n---\n\n"
            )
            filename = "/".join(filter(None, [slug, page["slug"]])) + ".md"
            pages[filename] = header + text
    metadata[""]["pages"] = root_order
    # Validate everything before replacing the previous generated collection.
    output = root / "website/.generated-docs"
    staging = Path(tempfile.mkdtemp(prefix=".generated-docs-", dir=output.parent))
    try:
        for filename, text in pages.items():
            target = staging / filename
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text)
        for slug, meta in metadata.items():
            target = staging / slug / "meta.json"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(json.dumps(meta, indent=2) + "\n")
        if output.exists():
            shutil.rmtree(output)
        staging.rename(output)
    finally:
        if staging.exists():
            shutil.rmtree(staging)
    return len(pages)


if __name__ == "__main__":
    print(f"Prepared {prepare(ROOT)} documentation pages from repository Markdown.")
