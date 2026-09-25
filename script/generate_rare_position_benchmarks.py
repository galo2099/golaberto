#!/usr/bin/env python3
"""Create a fixed synthetic 20-team suite when saved production groups are unavailable."""

import argparse
import json
from pathlib import Path


def strength(scenario, team):
    if scenario == "tight":
        return 1.25 + (team % 3 - 1) * 0.025
    if scenario == "wide":
        return 0.55 + (19 - team) * 0.095
    if scenario == "dominant":
        return 3.2 if team == 0 else 1.1 + (team % 4) * 0.04
    if scenario == "weak":
        return 0.3 if team == 19 else 1.1 + (team % 4) * 0.04
    if scenario == "blockers":
        return 1.5 if team in (8, 9, 10, 11) else 0.75 + (19 - team) * 0.075
    raise ValueError(scenario)


def make_group(group_id, scenario):
    games = []
    game_id = 1
    for leg in range(2):
        for home in range(20):
            for away in range(home + 1, 20):
                h, a = (home, away) if leg == 0 else (away, home)
                h_power = round(1.12 * strength(scenario, h), 4)
                a_power = round(0.88 * strength(scenario, a), 4)
                played = game_id <= 50
                game = {
                    "id": game_id,
                    "home_id": h + 1,
                    "away_id": a + 1,
                    "home_power": h_power,
                    "away_power": a_power,
                    "played": played,
                }
                if played:
                    # Deterministic standings; no random seed or external data.
                    game["home_score"] = round(h_power)
                    game["away_score"] = round(a_power)
                games.append(game)
                game_id += 1
    return {
        "id": group_id,
        "phase": {
            "sort": "pt,gd,gf,bias",
            "championship": {"point_win": 3, "point_draw": 1, "point_loss": 0},
        },
        "team_groups": [{"team_id": i + 1, "bias": i, "add_sub": 0} for i in range(20)],
        "games": games,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for index, scenario in enumerate(("tight", "wide", "dominant", "weak", "blockers"), 1):
        path = args.output / f"group-{99000 + index}-{scenario}.json"
        path.write_text(json.dumps(make_group(99000 + index, scenario), separators=(",", ":")) + "\n")
        print(path)


if __name__ == "__main__":
    main()
