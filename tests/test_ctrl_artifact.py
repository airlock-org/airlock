"""GUI review tests must reject stale source or a substituted build output."""

import json
from pathlib import Path

import pytest

import ctrl_artifact


@pytest.fixture
def artifact(tmp_path):
    for name in (
        "apps/Cargo.toml",
        "apps/Cargo.lock",
        "apps/airlock-ctrl-gui/Cargo.toml",
        "apps/airlock-ctrl-gui/src/main.rs",
        "apps/airlock-ctrl-gui/assets/logo.svg",
    ):
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(name)
    binary = tmp_path / "airlock-ctrl-gui"
    binary.write_bytes(b"built binary")
    receipt = tmp_path / "build.json"
    receipt.write_text(
        json.dumps(
            {
                "binary": str(binary),
                "binary_sha256": ctrl_artifact.file_digest(binary),
                "source_sha256": ctrl_artifact.source_digest(tmp_path),
            }
        )
    )
    return tmp_path, binary, receipt


def test_current_build_is_accepted(artifact):
    root, binary, receipt = artifact
    assert ctrl_artifact.verify(receipt, root=root) == binary


@pytest.mark.parametrize("changed", ["src/main.rs", "assets/logo.svg", "Cargo.toml"])
def test_changed_gui_input_requires_explicit_rebuild(artifact, changed):
    root, _, receipt = artifact
    (root / "apps/airlock-ctrl-gui" / changed).write_text("changed")
    with pytest.raises(ValueError, match="sources changed"):
        ctrl_artifact.verify(receipt, root=root)


def test_substituted_gui_binary_is_rejected(artifact):
    root, binary, receipt = artifact
    binary.write_bytes(b"different binary")
    with pytest.raises(ValueError, match="binary changed"):
        ctrl_artifact.verify(receipt, root=root)


def test_explicit_binary_must_match_verified_build(artifact):
    root, _, receipt = artifact
    with pytest.raises(ValueError, match="differs from the verified build"):
        ctrl_artifact.verify(receipt, configured=Path("/other-build"), root=root)
