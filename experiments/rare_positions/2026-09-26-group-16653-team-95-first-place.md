# Group 16653: team 95 can finish first

The local `GolAberto_development` snapshot of group 16653 has 20 teams, 290 played games, and 90 remaining games (nine per team). Team 95 has 28 points; the current leaders, teams 22 and 70, have 51. The group awards three points for a win and one for a draw, with no team point adjustments.

The accompanying [fixture and outcome witness](2026-09-26-group-16653-team-95-first-place-witness.json) assigns one result to each of the 90 remaining games. Team 95 wins all nine of its games and finishes on **55 points**. The highest rival total is **54 points**, shared by teams 22, 70, 77, and 588. Thus team 95 finishes first outright on points, regardless of tiebreakers.

The witness uses 26 home wins, 33 draws, and 31 away wins across the remaining fixtures. Every remaining game's two Poisson means are positive in the live group request, so each assigned win or draw has positive probability. The product of the 90 outcome probabilities for this one schedule is approximately `9.23e-53`. This is a valid but extremely loose lower bound on team 95's first-place probability, **not** a usable odds estimate.

To verify the point result from the captured fixture snapshot:

```python
import json
from collections import defaultdict

data = json.load(open("experiments/rare_positions/2026-09-26-group-16653-team-95-first-place-witness.json"))
points = defaultdict(int)
remaining = 0
for game in data["games"]:
    home, away = game["home_id"], game["away_id"]
    if "played_score" in game:
        home_goals, away_goals = game["played_score"]
        outcome = "home_win" if home_goals > away_goals else "away_win" if away_goals > home_goals else "draw"
    else:
        remaining += 1
        outcome = game["witness_outcome"]
    if outcome == "home_win":
        points[home] += 3
    elif outcome == "away_win":
        points[away] += 3
    else:
        points[home] += 1
        points[away] += 1

assert remaining == 90
assert points[95] == 55
assert max(value for team, value in points.items() if team != 95) == 54
```

The currently stored first-place odds for team 95 are zero. That value reflects a missing probability estimate, not impossibility. A reachability witness should be recorded separately from the estimated probability.

`TestGroup16653Team95FirstPlaceWitness` in `go/rare_position_reachability_witness_test.go` also applies this fixture to the Go `TeamCampaign` standings and sorter. It verifies that Go's conservative bounds do not rule out first place and that the completed witness ranks team 95 first, 55 to 54. Production Go code does not yet search for or persist such a witness automatically.
