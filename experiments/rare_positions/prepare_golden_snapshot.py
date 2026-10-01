#!/usr/bin/env python3
"""Reopen the latest played fixtures in a private Rails export for a saved benchmark.

Fixtures, powers and their ordering remain intact. This is a controlled input
transformation, not a historical reconstruction of team ratings or a DB update.
"""

import argparse
import copy
import hashlib
import json
from pathlib import Path


def unplay_latest_games(raw, count):
    if count < 0:
        raise ValueError("removed game count must be nonnegative")
    games = raw["games"]
    if len({game["id"] for game in games}) != len(games):
        raise ValueError("duplicate fixture IDs")
    played = [game for game in games if game["played"]]
    if count > len(played):
        raise ValueError("not enough played games to remove")
    if any(not game.get("date") for game in played):
        raise ValueError("export game dates to select the latest results")
    selected = sorted(played, key=lambda game: (game["date"], game["id"]), reverse=True)[:count]
    ids = {game["id"] for game in selected}
    request = copy.deepcopy(raw)
    for game in request["games"]:
        if game["id"] in ids:
            game.update(played=False, home_score=0, away_score=0)
    request["dataset_transform"] = {
        "method": "unplay_latest_played_games",
        "removed_played_games": count,
        "removed_game_ids": [game["id"] for game in selected],
        "selection_order": "date descending, then fixture ID descending",
        "source_played_games": len(played),
        "resulting_played_games": len(played) - count,
        "remaining_games": len(games) - len(played) + count,
        "fixture_powers": "unchanged from source export",
    }
    return request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--remove-played", type=int, default=200)
    args = parser.parse_args()
    source = args.source.read_bytes()
    request = unplay_latest_games(json.loads(source), args.remove_played)
    request["dataset_transform"]["source_input_sha256"] = hashlib.sha256(source).hexdigest()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(request, ensure_ascii=False, indent=2, allow_nan=False) + "\n")
    print(json.dumps({"phase": request["phase"]["id"], "group": request["id"],
                      "remaining_games": request["dataset_transform"]["remaining_games"]}))


if __name__ == "__main__":
    main()
