import argparse
import os


from .core import (
    DEFAULT_COMPACTION_MAX_SEGMENTS,
    DEFAULT_COMPACTION_THRESHOLD_PERCENT,
    DEFAULT_VIEW_BACKEND,
    VIEW_BACKEND_EROFS_OBJECTMAP,
    default_physical_store_root,
)
from .generations import (
    admit_generation,
    clear_running,
    mark_running,
    rollback,
    set_retention,
    switch,
)
from .maintenance import compact, gc, recover, status
from .views import attach_blob_view, detach_blob_view


def add_common(parser):
    parser.add_argument(
        "--pool-root",
        default=os.environ.get(
            "AIRLOCK_CLOSURE_STORE_ROOT", "/var/lib/airlock/closure-store"
        ),
    )


def build_parser():
    parser = argparse.ArgumentParser(prog="airlock-closure-store")
    sub = parser.add_subparsers(dest="command", required=True)

    admit_parser = sub.add_parser("admit")
    add_common(admit_parser)
    admit_parser.add_argument("--domain", required=True)
    admit_parser.add_argument("--system", required=True)
    admit_parser.add_argument("--closure-source", required=True)
    admit_parser.add_argument("--root-source", default="")
    admit_parser.add_argument("--kernel-name", default="")
    admit_parser.add_argument("--store-device", default="/dev/xvde")
    admit_parser.add_argument("--store-blob-device", default="/dev/xvdf")
    admit_parser.add_argument("--store-mountpoint", default="/nix/store")
    admit_parser.add_argument(
        "--physical-store-root", default=default_physical_store_root()
    )
    admit_parser.add_argument(
        "--segment-target-bytes",
        default=os.environ.get("AIRLOCK_CLOSURE_STORE_SEGMENT_TARGET_BYTES", ""),
        help="target maximum size for open immutable object segments",
    )
    admit_parser.add_argument(
        "--verify-mode",
        default=os.environ.get("AIRLOCK_CLOSURE_STORE_VERIFY", "nix-store"),
        choices=["none", "tree", "nix-store", "nix-store-trust"],
    )
    admit_parser.add_argument(
        "--view-backend",
        default=os.environ.get(
            "AIRLOCK_CLOSURE_STORE_VIEW_BACKEND", DEFAULT_VIEW_BACKEND
        ),
        choices=[VIEW_BACKEND_EROFS_OBJECTMAP],
    )
    admit_parser.add_argument("--approved-by", default="dom0")
    admit_parser.add_argument("--approval-source", default="")
    admit_parser.add_argument(
        "--activate",
        default="true",
        choices=["true", "false"],
        help="whether to atomically select the new generation after admission",
    )
    admit_parser.add_argument(
        "--retain",
        default="true",
        choices=["true", "false"],
        help="whether GC must retain this generation even after it stops being current",
    )
    admit_parser.set_defaults(func=admit_generation)

    rollback_parser = sub.add_parser("rollback")
    add_common(rollback_parser)
    rollback_parser.add_argument("--domain", required=True)
    rollback_parser.add_argument("--generation", required=True)
    rollback_parser.set_defaults(func=rollback)

    retention_parser = sub.add_parser("set-retention")
    add_common(retention_parser)
    retention_parser.add_argument("--domain", required=True)
    retention_parser.add_argument("--generation", required=True)
    retention_parser.add_argument("--retain", required=True, choices=["true", "false"])
    retention_parser.set_defaults(func=set_retention)

    switch_parser = sub.add_parser("switch")
    add_common(switch_parser)
    switch_parser.add_argument("--domain", required=True)
    switch_parser.add_argument("--generation", required=True)
    switch_parser.set_defaults(func=switch)

    gc_parser = sub.add_parser("gc")
    add_common(gc_parser)
    gc_parser.add_argument("--keep-generations", type=int, default=2)
    gc_parser.set_defaults(func=gc)

    compact_parser = sub.add_parser("compact")
    add_common(compact_parser)
    compact_parser.add_argument(
        "--dry-run",
        default="false",
        choices=["true", "false"],
        help="report compaction candidates without moving objects",
    )
    compact_parser.add_argument(
        "--threshold-percent",
        default=str(DEFAULT_COMPACTION_THRESHOLD_PERCENT),
        help="compact sealed non-oversized segments at or below this live-byte utilization",
    )
    compact_parser.add_argument(
        "--max-segments",
        default=str(DEFAULT_COMPACTION_MAX_SEGMENTS),
        help="maximum candidate segments to compact in one bounded run",
    )
    compact_parser.add_argument(
        "--keep-generations",
        type=int,
        default=2,
        help="latest generations per domain to treat as live for compaction planning",
    )
    compact_parser.add_argument(
        "--segment-target-bytes",
        default=os.environ.get("AIRLOCK_CLOSURE_STORE_SEGMENT_TARGET_BYTES", ""),
        help="target maximum size for replacement object segments",
    )
    compact_parser.set_defaults(func=compact)

    recover_parser = sub.add_parser("recover")
    add_common(recover_parser)
    recover_parser.set_defaults(func=recover)

    status_parser = sub.add_parser("status")
    add_common(status_parser)
    status_parser.set_defaults(func=status)

    attach_parser = sub.add_parser("attach-blob-view")
    add_common(attach_parser)
    attach_parser.add_argument("--domain", required=True)
    attach_parser.add_argument("--generation", required=True)
    attach_parser.set_defaults(
        func=lambda args: attach_blob_view(args.pool_root, args.domain, args.generation)
    )

    detach_parser = sub.add_parser("detach-blob-view")
    add_common(detach_parser)
    detach_parser.add_argument("--domain", required=True)
    detach_parser.add_argument("--generation", required=True)
    detach_parser.set_defaults(
        func=lambda args: detach_blob_view(args.pool_root, args.domain, args.generation)
    )

    mark_running_parser = sub.add_parser("mark-running")
    add_common(mark_running_parser)
    mark_running_parser.add_argument("--domain", required=True)
    mark_running_parser.add_argument("--generation", required=True)
    mark_running_parser.add_argument("--volume", required=True)
    mark_running_parser.set_defaults(func=mark_running)

    clear_running_parser = sub.add_parser("clear-running")
    add_common(clear_running_parser)
    clear_running_parser.add_argument("--domain", required=True)
    clear_running_parser.add_argument("--generation", required=True)
    clear_running_parser.add_argument("--volume", required=True)
    clear_running_parser.set_defaults(func=clear_running)

    return parser


def run(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)
