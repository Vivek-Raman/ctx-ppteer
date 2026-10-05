#!/usr/bin/env python3
"""Print the Markdown path currently configured for ctx-ppteer."""

import json
import os
import sys
from pathlib import Path


settings_path = Path(
    os.environ.get(
        "CTX_PPTEER_SETTINGS_PATH",
        Path.home() / ".config" / "ctx-ppteer" / "settings.json",
    )
)

try:
    settings = json.loads(settings_path.read_text(encoding="utf-8"))
    source_path = settings["sourcePath"]
except (FileNotFoundError, json.JSONDecodeError, KeyError, OSError) as error:
    print(f"Unable to read ctx-ppteer settings at {settings_path}: {error}", file=sys.stderr)
    raise SystemExit(1)

if not isinstance(source_path, str) or not source_path.strip():
    print(f"ctx-ppteer settings at {settings_path} contain no sourcePath", file=sys.stderr)
    raise SystemExit(1)

print(source_path)
