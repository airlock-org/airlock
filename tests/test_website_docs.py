"""Documentation publication checks; exported-site checks are explicitly opt-in."""

from __future__ import annotations

from html.parser import HTMLParser
import importlib.util
import json
import os
from pathlib import Path
from urllib.parse import unquote, urlsplit

import pytest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "website_docs", ROOT / "website/scripts/prepare-docs.py"
)
assert SPEC and SPEC.loader
DOCS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DOCS)


@pytest.fixture
def repository(tmp_path):
    (tmp_path / "website").mkdir()
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/overview.md").write_text(
        "# Overview\n\nSee [guide](guide.md#details).\n"
    )
    (tmp_path / "docs/guide.md").write_text("# Guide\n\n## Details\n\nContent.\n")
    (tmp_path / "docs/private.md").write_text("# Unpublished\n")
    config = {
        "groups": [
            {
                "slug": "",
                "title": "Start",
                "pages": [
                    {
                        "source": "docs/overview.md",
                        "slug": "index",
                        "title": "Overview",
                        "description": "Start here.",
                    },
                ],
            },
            {
                "slug": "guides",
                "title": "Guides",
                "pages": [
                    {
                        "source": "docs/guide.md",
                        "slug": "guide",
                        "title": "Guide",
                        "description": "Operating guide.",
                    },
                ],
            },
        ]
    }
    (tmp_path / "website/docs.config.json").write_text(json.dumps(config))
    return tmp_path


def test_preparation_publishes_only_selected_pages_and_preserves_sources(repository):
    originals = {p: p.read_bytes() for p in (repository / "docs").glob("*.md")}
    assert DOCS.prepare(repository) == 2
    output = repository / "website/.generated-docs"
    assert {p.relative_to(output).as_posix() for p in output.rglob("*.md")} == {
        "index.md",
        "guides/guide.md",
    }
    assert "[guide](/docs/guides/guide/#details)" in (output / "index.md").read_text()
    assert "# Overview\n" not in (output / "index.md").read_text()
    assert "## Details" in (output / "guides/guide.md").read_text()
    assert json.loads((output / "meta.json").read_text())["pages"] == [
        "index",
        "guides",
    ]
    assert all(p.read_bytes() == original for p, original in originals.items())


def test_failed_preparation_preserves_previous_collection(repository):
    DOCS.prepare(repository)
    output = repository / "website/.generated-docs/index.md"
    previous = output.read_bytes()
    (repository / "docs/overview.md").write_text(
        "# Overview\n\n[missing](missing.md)\n"
    )
    with pytest.raises(ValueError, match="Broken repository link"):
        DOCS.prepare(repository)
    assert output.read_bytes() == previous


def test_regeneration_removes_pages_no_longer_published(repository):
    DOCS.prepare(repository)
    manifest = repository / "website/docs.config.json"
    config = json.loads(manifest.read_text())
    config["groups"].pop()
    manifest.write_text(json.dumps(config))
    DOCS.prepare(repository)
    assert not (repository / "website/.generated-docs/guides").exists()


def test_page_and_folder_index_cannot_publish_the_same_url(repository):
    manifest = repository / "website/docs.config.json"
    config = json.loads(manifest.read_text())
    config["groups"][0]["pages"][0]["slug"] = "guides"
    config["groups"][1]["pages"][0]["slug"] = "index"
    manifest.write_text(json.dumps(config))
    with pytest.raises(ValueError, match="Repeated source or destination"):
        DOCS.prepare(repository)


@pytest.mark.parametrize(
    "target,expected",
    [
        ("guide.md#details", "/docs/guides/guide/#details"),
        (
            "private.md",
            "https://github.com/airlock-org/airlock/blob/main/docs/private.md",
        ),
        ("./", "https://github.com/airlock-org/airlock/tree/main/docs"),
        ("#details", "#details"),
        ("https://example.com/a", "https://example.com/a"),
        ("mailto:example@example.com", "mailto:example@example.com"),
    ],
)
def test_link_destinations(repository, target, expected):
    _, routes = DOCS.publication(repository)
    assert (
        DOCS.rewrite_target(repository, repository / "docs/overview.md", target, routes)
        == expected
    )


def test_multiline_reference_and_code_links(repository):
    _, routes = DOCS.publication(repository)
    text = (
        '[multi\nline](guide.md#details "title")\n'
        "[label with `code`](guide.md)\n"
        '[ref]: guide.md#details "title"\n'
        "`[literal](missing.md)` and ``[also literal](missing.md)``\n"
        "```markdown\n[example](missing.md)\n```\n"
        "~~~markdown\n[example](missing.md)\n~~~\n"
    )
    result = DOCS.rewrite_markdown(
        repository, repository / "docs/overview.md", text, routes
    )
    assert '[multi\nline](/docs/guides/guide/#details "title")' in result
    assert "[label with `code`](/docs/guides/guide/)" in result
    assert '[ref]: /docs/guides/guide/#details "title"' in result
    assert result.count("[example](missing.md)") == 2
    assert "`[literal](missing.md)`" in result
    assert "``[also literal](missing.md)``" in result


@pytest.mark.parametrize(
    "markdown,expected",
    [
        (
            "![VM topology](../website/public/diagrams/vm-topology.svg)",
            "![VM topology](/diagrams/vm-topology.svg)",
        ),
        (
            "[Full-size diagram](../website/public/diagrams/vm-topology.svg#title)",
            "[Full-size diagram](/diagrams/vm-topology.svg#title)",
        ),
        (
            "[topology]: ../website/public/diagrams/vm-topology.svg",
            "[topology]: /diagrams/vm-topology.svg",
        ),
    ],
)
def test_public_assets_use_local_export_urls(repository, markdown, expected):
    asset = repository / "website/public/diagrams/vm-topology.svg"
    asset.parent.mkdir(parents=True)
    asset.write_text('<svg xmlns="http://www.w3.org/2000/svg"/>')
    assert (
        DOCS.rewrite_markdown(repository, repository / "docs/overview.md", markdown, {})
        == expected
    )


def test_component_source_links_work_without_component_checkout(repository):
    source = repository / "docs/overview.md"
    target = "../airlock-machine/flake.nix"
    assert DOCS.rewrite_target(repository, source, target, {}) == (
        "https://github.com/airlock-org/machine/blob/main/flake.nix"
    )
    (repository / "airlock-machine").mkdir()
    (repository / "airlock-machine/.git").write_text(
        "gitdir: ../.git/modules/airlock-machine"
    )
    with pytest.raises(ValueError, match="Broken repository link"):
        DOCS.rewrite_target(repository, source, target, {})


@pytest.mark.parametrize(
    "source,slug",
    [
        ("docs/missing.md", "index"),
        ("../outside.md", "index"),
        ("docs/overview.md", "../../outside"),
    ],
)
def test_invalid_publication_paths_are_rejected(repository, source, slug):
    manifest = repository / "website/docs.config.json"
    config = json.loads(manifest.read_text())
    page = config["groups"][0]["pages"][0]
    page.update(source=source, slug=slug)
    manifest.write_text(json.dumps(config))
    with pytest.raises(ValueError):
        DOCS.prepare(repository)
    assert not (repository / "website/.generated-docs").exists()


def test_current_publication_has_valid_repository_links():
    _, routes = DOCS.publication(ROOT)
    for source in routes:
        DOCS.rewrite_markdown(ROOT, source, source.read_text(), routes)


class HTMLPage(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.links = []
        self.images = []
        self.ids = set()
        self.headings = 0
        self.feed(text)

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if tag == "a" and "href" in attrs:
            self.links.append(attrs["href"])
        if tag == "img" and "src" in attrs:
            self.images.append(attrs["src"])
        if "id" in attrs:
            self.ids.add(attrs["id"])
        if tag == "h1":
            self.headings += 1


@pytest.fixture
def exported_site():
    value = os.environ.get("AIRLOCK_WEBSITE_OUT")
    if not value:
        pytest.skip("Set AIRLOCK_WEBSITE_OUT after an explicit website build")
    output = Path(value).resolve()
    assert (output / "index.html").is_file(), (
        "Build the website before checking its export"
    )
    return output


def test_export_contains_every_published_page_and_static_search(exported_site):
    _, routes = DOCS.publication(ROOT)
    for route in ["/", *routes.values()]:
        page = exported_site / route.lstrip("/") / "index.html"
        assert page.is_file(), route
        assert HTMLPage(page.read_text()).headings == 1, route
    assert (exported_site / "404.html").is_file()
    index = json.loads((exported_site / "docs/search.json").read_text())
    assert index["type"] == "advanced"
    indexed = set(index["internalDocumentIDStore"]["internalIdToId"])
    assert all(route.rstrip("/") in indexed for route in routes.values())


def test_export_internal_links_and_fragments_resolve(exported_site):
    pages = {path: HTMLPage(path.read_text()) for path in exported_site.rglob("*.html")}
    failures = []
    for source, page in pages.items():
        for href in page.links:
            url = urlsplit(href)
            if url.scheme or url.netloc:
                continue
            if not url.path:
                target = source
            elif url.path.startswith("/"):
                target = exported_site / unquote(url.path).lstrip("/")
            else:
                target = source.parent / unquote(url.path)
            if target.is_dir():
                target /= "index.html"
            if not target.is_file():
                failures.append(f"{source.relative_to(exported_site)}: missing {href}")
            elif (
                url.fragment
                and target in pages
                and unquote(url.fragment) not in pages[target].ids
            ):
                failures.append(
                    f"{source.relative_to(exported_site)}: missing anchor {href}"
                )
    assert not failures, "\n".join(failures)


@pytest.mark.parametrize("filename", ["network-topology.svg", "gui-topology.svg"])
def test_export_documentation_images_are_local_and_present(exported_site, filename):
    architecture = HTMLPage(
        (exported_site / "docs/architecture/index.html").read_text()
    )
    for path in (exported_site / "docs").rglob("*.html"):
        for src in HTMLPage(path.read_text()).images:
            url = urlsplit(src)
            assert not url.scheme and not url.netloc, (path, src)
            assert url.path.startswith("/"), (path, src)
            assert (exported_site / unquote(url.path).lstrip("/")).is_file(), (
                path,
                src,
            )
    diagram = (ROOT / "website/public/diagrams" / filename).read_bytes()
    assert any(
        (exported_site / unquote(urlsplit(src).path).lstrip("/")).read_bytes()
        == diagram
        for src in architecture.images
    )
