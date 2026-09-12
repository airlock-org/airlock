import json
import sys

from .engine import EngineError, run


def main(argv=None):
    try:
        result = run(argv)
    except EngineError as exc:
        print(f"airlock-closure-store: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
