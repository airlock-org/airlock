#!/usr/bin/env python3
"""Offline VFCT inspection/preparation; runtime shares the same validator."""

from pathlib import Path
import sys

sys.path.insert(
    0,
    str(
        Path(__file__).resolve().parents[2] / "runtime/tools/pkgs/airlock-amd-vbios/src"
    ),
)
from airlock_amd_vbios.firmware import main  # noqa: E402

if __name__ == "__main__":
    main()
