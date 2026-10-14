"""Exercise the actual folding/navigation module without a display or Nix build."""

from __future__ import annotations

import json
from pathlib import Path
import shutil
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]


@pytest.fixture(scope="module")
def context_runner(tmp_path_factory):
    compiler = shutil.which("rustc")
    if not compiler:
        pytest.skip("Rust compiler is not installed; no automatic provisioning")
    directory = tmp_path_factory.mktemp("ctrl-context")
    harness = directory / "main.rs"
    module = ROOT / "apps/airlock-ctrl/src/diff_context.rs"
    harness.write_text(
        "#[path = "
        + json.dumps(str(module))
        + "] mod diff_context;\n"
        + r"""
use diff_context::{ContextView, DisplayRow, adjacent_change};
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments[0] == "navigate" {
        let changes = [(0, 10), (0, 11), (2, 3), (4, 8)];
        let current = arguments[1].parse().ok();
        let file = arguments[2].parse().unwrap();
        let forward = arguments[3] == "next";
        match adjacent_change(&changes, current, file, forward) {
            Some(index) => println!("{index}"),
            None => println!("null"),
        }
        return;
    }
    let mut view = ContextView::new(arguments[0].chars().map(|c| c == 'X').collect());
    for action in &arguments[1..] {
        match action.as_str() {
            "all" => view.show_all(),
            "reset" => view.reset(),
            _ => {
                let parts: Vec<_> = action.split(':').collect();
                view.expand(parts[1].parse().unwrap(), parts[2].parse().unwrap(), parts[0] == "all");
            },
        }
    }
    let rows: Vec<_> = view.entries.iter().map(|row| match row {
        DisplayRow::Source(index) => format!("{index}"),
        DisplayRow::Gap { start, end } => format!("[{start},{end}]"),
    }).collect();
    println!("[{}]", rows.join(","));
}
"""
    )
    binary = directory / "context"
    subprocess.run(
        [
            compiler,
            "--edition=2024",
            "-A",
            "dead_code",
            str(harness),
            "-o",
            str(binary),
        ],
        check=True,
        capture_output=True,
        text=True,
    )

    def run(*arguments):
        result = subprocess.run(
            [str(binary), *map(str, arguments)],
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(result.stdout)

    return run


def flags(count, *changes):
    return "".join("X" if index in changes else "." for index in range(count))


def test_context_keeps_changed_lines_and_merges_nearby_sections(context_runner):
    assert context_runner(flags(40, 10, 15, 30)) == [
        [0, 7],
        *range(7, 19),
        [19, 27],
        *range(27, 34),
        [34, 40],
    ]


def test_context_handles_additions_deletions_boundaries_and_metadata_only(
    context_runner,
):
    assert context_runner("XXXX") == [0, 1, 2, 3]
    assert context_runner(flags(20, 0, 19)) == [0, 1, 2, 3, [4, 16], 16, 17, 18, 19]
    assert context_runner("....") == [[0, 4]]
    assert context_runner("") == []


def test_middle_expansion_reveals_context_at_both_ends_without_losing_rows(
    context_runner,
):
    source = flags(160, 10, 140)
    initial = context_runner(source)
    assert [14, 137] in initial
    expanded = context_runner(source, "more:14:137")
    assert expanded == [
        [0, 7],
        *range(7, 34),
        [34, 117],
        *range(117, 144),
        [144, 160],
    ]
    assert context_runner(source, "all:14:137") == [
        [0, 7],
        *range(7, 144),
        [144, 160],
    ]
    assert context_runner(source, "more:14:137", "reset") == initial
    assert context_runner(source, "all") == list(range(160))


def test_prefix_and_suffix_expand_toward_the_change(context_runner):
    source = flags(100, 50)
    assert context_runner(source, "more:0:47") == [[0, 27], *range(27, 54), [54, 100]]
    assert context_runner(source, "more:54:100") == [[0, 47], *range(47, 74), [74, 100]]
    assert context_runner(source, "more:0:47", "more:0:27", "more:0:7") == [
        *range(54),
        [54, 100],
    ]


@pytest.mark.parametrize(
    "current,file,direction,expected",
    [
        (0, 0, "previous", None),
        (0, 0, "next", 1),
        (1, 0, "next", 2),
        (2, 2, "previous", 1),
        (3, 4, "next", None),
        ("none", 1, "next", 2),
        ("none", 1, "previous", 1),
        ("none", 5, "next", None),
        ("none", 0, "next", 0),
    ],
)
def test_changed_line_navigation_crosses_files_and_skips_metadata_only_files(
    context_runner,
    current,
    file,
    direction,
    expected,
):
    assert context_runner("navigate", current, file, direction) == expected
