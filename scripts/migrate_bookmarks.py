#!/usr/bin/env python3
"""
Bookmark migration script for MandelbRust Phase 16.

Upgrades bookmark JSON files to include the full DisplayColorSettings
structure with all Phase 16 fields (coloring_mode, interior_mode,
stripe_density, custom_palette_name).

Usage:
    python migrate_bookmarks.py [bookmarks_directory]

If no directory is given, defaults to ./bookmarks/ relative to the
script's location.

Old bookmarks that only have palette_index/smooth_coloring (without a
display_color block) will gain a full display_color section. Bookmarks
that already have display_color will gain any missing Phase 16 fields
with their default values.

The script modifies files in place but creates a .bak backup for each
file it changes.
"""

import json
import os
import shutil
import sys

DISPLAY_COLOR_DEFAULTS = {
    "palette_index": 0,
    "palette_mode": {"mode": "by_cycles", "n": 1},
    "start_from": "none",
    "low_threshold_start": 10,
    "low_threshold_end": 30,
    "smooth_coloring": True,
    "coloring_mode": "standard",
    "interior_mode": "black",
    "stripe_density": 1.0,
    # custom_palette_name is intentionally omitted (null / absent = use builtin)
}


def migrate_bookmark(data: dict) -> bool:
    """Migrate a single bookmark dict. Returns True if modified."""
    changed = False

    if data.get("display_color") is None:
        dc = dict(DISPLAY_COLOR_DEFAULTS)
        dc["palette_index"] = data.get("palette_index", 0)
        dc["smooth_coloring"] = data.get("smooth_coloring", True)
        data["display_color"] = dc
        changed = True
    else:
        dc = data["display_color"]
        for key, default in DISPLAY_COLOR_DEFAULTS.items():
            if key not in dc:
                dc[key] = default
                changed = True

    return changed


def main():
    if len(sys.argv) > 1:
        bookmarks_dir = sys.argv[1]
    else:
        script_dir = os.path.dirname(os.path.abspath(__file__))
        bookmarks_dir = os.path.join(script_dir, "..", "bookmarks")

    bookmarks_dir = os.path.abspath(bookmarks_dir)

    if not os.path.isdir(bookmarks_dir):
        print(f"Directory not found: {bookmarks_dir}")
        sys.exit(1)

    files = [f for f in os.listdir(bookmarks_dir) if f.endswith(".json")]
    if not files:
        print(f"No .json files found in {bookmarks_dir}")
        return

    migrated = 0
    skipped = 0
    errors = 0

    for filename in sorted(files):
        path = os.path.join(bookmarks_dir, filename)
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except (json.JSONDecodeError, OSError) as e:
            print(f"  ERROR reading {filename}: {e}")
            errors += 1
            continue

        if migrate_bookmark(data):
            backup = path + ".bak"
            shutil.copy2(path, backup)
            with open(path, "w", encoding="utf-8") as f:
                json.dump(data, f, indent=2, ensure_ascii=False)
                f.write("\n")
            print(f"  MIGRATED {filename} (backup: {filename}.bak)")
            migrated += 1
        else:
            skipped += 1

    print(f"\nDone. {migrated} migrated, {skipped} already up-to-date, {errors} errors.")


if __name__ == "__main__":
    main()
