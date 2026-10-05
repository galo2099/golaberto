#!/usr/bin/env python3
"""Aggregate opt-in lazy sampler hit audits; individual hits are contributions, not probabilities."""
import argparse
import json
import math
from collections import defaultdict
from pathlib import Path


def status(hit, team_id):
    for label, key in (("above", "strictly_above"), ("equal", "equal"), ("below", "strictly_below")):
        if team_id in hit["rivals"][key]:
            return label
    return "not_present_in_synthetic_fixture"


def aggregate(streams):
    reports = []
    for stream in sorted(streams, key=lambda s: (s["stream_seed"], s["sampler_result_summary"]["samples"])):
        samples = stream["sampler_result_summary"]["samples"]
        mass = stream["sampler_result_summary"]["mass"]
        target = defaultdict(lambda: {"raw_hits": 0, "weight_sum": 0.0})
        rivalry = defaultdict(lambda: {"raw_hits": 0, "weight_sum": 0.0})
        score_sampled = {"raw_hits": 0, "weight_sum": 0.0}
        goal_independent = {"raw_hits": 0, "weight_sum": 0.0}
        score_sampling_unknown = {"raw_hits": 0, "weight_sum": 0.0}
        for hit in stream["all_positive_hits"]:
            w = hit["corrected_weight"]
            target_key = (hit["target_added_points"], hit["target_added_wins"])
            target[target_key]["raw_hits"] += 1
            target[target_key]["weight_sum"] += w
            rivalry_key = (tuple(sorted(hit["rivals"]["strictly_above"])),
                           tuple(sorted(hit["rivals"]["equal"])),
                           tuple(sorted(hit["rivals"]["strictly_below"])))
            rivalry[rivalry_key]["raw_hits"] += 1
            rivalry[rivalry_key]["weight_sum"] += w
            flag = hit.get("score_sampled")
            bucket = score_sampled if flag is True else goal_independent if flag is False else score_sampling_unknown
            bucket["raw_hits"] += 1
            bucket["weight_sum"] += w
        team_rows = []
        for team in stream.get("team_final_points_wins_histograms", []):
            team_rows.append({"team_id": team["team_id"], "final_points_wins": [
                {"points": bin_["points"], "wins": bin_["wins"], "raw_hits": bin_["raw_hits"],
                 "estimated_probability_contribution": mass * bin_["corrected_weight_sum"] / max(samples, 1)}
                for bin_ in team["final_points_wins"]]})
        fixture_rows = []
        for fixture in stream.get("fixture_wdl_marginals", []):
            names = ("away_win", "draw", "home_win")
            weighted = fixture["weighted_corrected_sum_by_wdl"]
            total = sum(weighted)
            fixture_rows.append({"game_id": fixture["game_id"], "home_id": fixture["home_id"],
                "away_id": fixture["away_id"], "prior_wdl": fixture["prior_wdl"],
                "raw_positive_hits_by_wdl": dict(zip(names, fixture["raw_positive_hits_by_wdl"])),
                "estimated_probability_contribution_by_wdl": {
                    name: mass * weighted[i] / max(samples, 1) for i, name in enumerate(names)},
                "weighted_share_within_stream_by_wdl": {
                    name: weighted[i] / total if total else 0.0 for i, name in enumerate(names)}})
        top = []
        for hit in stream["top_32_corrected_weight_hits"]:
            top.append({"stream_seed": stream["stream_seed"], "draw_index": hit["draw_index"],
                "corrected_weight": hit["corrected_weight"],
                "monte_carlo_contribution": mass * hit["corrected_weight"] / max(samples, 1),
                "target_added_points": hit["target_added_points"], "target_added_wins": hit["target_added_wins"],
                "rivals": hit["rivals"], "score_sampled": hit["score_sampled"],
                "outcomes": hit["outcomes"], "scores": hit.get("scores"),
                "actual_target_campaign": hit.get("actual_target_campaign"),
                "team_points_wins": hit["team_points_wins"]})
        top.sort(key=lambda hit: (-hit["monte_carlo_contribution"], hit["stream_seed"], hit["draw_index"]))
        top = top[:32]
        samples = stream["sampler_result_summary"]["samples"]
        sum_weight = sum(h["corrected_weight"] for h in stream["all_positive_hits"])
        calculated = mass * (sum_weight / max(samples, 1))
        observed = stream["sampler_result_summary"]["probability"]
        def rows(table, target_rows=False):
            result = []
            for key, value in sorted(table.items()):
                record = {"raw_hits": value["raw_hits"],
                          "estimated_probability_contribution": mass * value["weight_sum"] / max(samples, 1)}
                if target_rows:
                    record.update({"target_added_points": key[0], "target_added_wins": key[1]})
                else:
                    rivalry_hit = {"rivals": {"strictly_above": key[0], "equal": key[1], "strictly_below": key[2]}}
                    record.update({"strictly_above_team_ids": list(key[0]), "equal_team_ids": list(key[1]),
                        "strictly_below_team_ids": list(key[2]),
                        "corinthians_24": status(rivalry_hit, 24), "mirassol_584": status(rivalry_hit, 584),
                        "vasco_18": status(rivalry_hit, 18), "botafogo_7": status(rivalry_hit, 7),
                        "vitoria_67": status(rivalry_hit, 67)})
                result.append(record)
            return result
        reports.append({"stream_seed": stream["stream_seed"], "source_label": stream.get("source_label"),
            "goal_tilt_used": stream.get("goal_tilt_used"),
            "mass": mass, "requested_draws": samples,
            "sampler_result_check": {"recorded_hits": len(stream["all_positive_hits"]),
                "sampler_hits": stream["sampler_result_summary"]["hits"],
                "mass_times_sum_over_samples": calculated, "sampler_probability": observed,
                "probability_matches": calculated == observed},
            "target_added_points_wins": rows(target, True), "rival_status_families": rows(rivalry),
            "per_team_final_points_wins": team_rows,
            "fixture_wdl_weighted_marginals": sorted(fixture_rows, key=lambda f: f["game_id"]),
            "score_sampling_coverage": {"score_sampled": {
                "raw_hits": score_sampled["raw_hits"], "estimated_probability_contribution": mass*score_sampled["weight_sum"]/max(samples,1)},
                "goal_independent": {"raw_hits":goal_independent["raw_hits"],"estimated_probability_contribution":mass*goal_independent["weight_sum"]/max(samples,1)},
                "unknown_without_goal_tilt": {"raw_hits":score_sampling_unknown["raw_hits"],"estimated_probability_contribution":mass*score_sampling_unknown["weight_sum"]/max(samples,1)}},
            "top_32_weighted_hits": top})
    return {"stream_count": len(reports), "streams": reports}


def self_test():
    def h(draw, points, wins, weight, above, equal, below):
        return {"draw_index": draw, "corrected_weight": weight, "target_added_points": points,
                "target_added_wins": wins, "score_sampled": False,
                "rivals": {"strictly_above": above, "equal": equal, "strictly_below": below}}
    def stream(seed, mass, n, weights):
        hits = [h(0, 3, 1, weights[0], [24, 18], [7], [584, 67]),
                h(1, 0, 0, weights[1], [7], [24, 18], [584, 67])]
        top = [{"draw_index": i, "corrected_weight": float(40-i), "target_added_points": i % 2,
                "target_added_wins": i % 2, "rivals": {"strictly_above": [], "equal": [], "strictly_below": []},
                "score_sampled": False, "outcomes": [], "scores": None,
                "actual_target_campaign": None, "team_points_wins": []} for i in range(40)]
        return {"stream_seed": seed, "sampler_result_summary": {"samples": n, "mass": mass,
            "hits": 2, "probability": mass * (sum(weights) / n)}, "all_positive_hits": hits,
            "fixture_wdl_marginals": [], "team_final_points_wins_histograms": [],
            "top_32_corrected_weight_hits": top}
    result = aggregate([stream(1, .5, 2, [1., 2.]), stream(2, .2, 4, [1., 3.])])
    assert result["stream_count"] == 2
    first, second = result["streams"]
    assert all(math.isclose(x["estimated_probability_contribution"], expected)
               for x, expected in zip(first["target_added_points_wins"], [.5, .25]))
    assert all(math.isclose(x["estimated_probability_contribution"], expected)
               for x, expected in zip(second["target_added_points_wins"], [.15, .05]))
    assert all(s["sampler_result_check"]["probability_matches"] for s in result["streams"])
    assert len(first["top_32_weighted_hits"]) == 32
    assert first["top_32_weighted_hits"][0]["draw_index"] == 0
    assert all({"strictly_above_team_ids", "equal_team_ids", "strictly_below_team_ids",
                "corinthians_24", "mirassol_584", "vasco_18", "botafogo_7", "vitoria_67"}
               <= row.keys() for s in result["streams"] for row in s["rival_status_families"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("request", nargs="?", type=Path)
    ap.add_argument("streams", nargs="?", type=Path)
    ap.add_argument("output", nargs="?", type=Path)
    ap.add_argument("--run-seed", type=int, help="top-level request seed, used to annotate derive tags")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test()
        print("self-test: ok")
        return
    if not args.request or not args.streams:
        ap.error("request and stream directory are required")
    request = json.loads(args.request.read_text())
    paths = sorted(args.streams.glob("stream-*.json"))
    streams = [json.loads(p.read_text()) for p in paths]
    if args.run_seed is not None:
        def derive(seed, tag):
            value = 14695981039346656037
            for byte in int(seed).to_bytes(8, "little", signed=True) + tag.encode():
                value = ((value ^ byte) * 1099511628211) & ((1 << 64) - 1)
            return value - (1 << 64) if value >> 63 else value
        labels = {}
        target_suffix = "17-11"
        for tag, label in ((f"rare-tail-main-{target_suffix}", "ordinary-main"),
                           (f"rare-tail-check-{target_suffix}", "ordinary-check"),
                           (f"rare-tail-neighbor-pilot-{target_suffix}", "native-transfer-pilot"),
                           (f"rare-tail-confirm-more-main-{target_suffix}", "confirm-more-main"),
                           (f"rare-tail-confirm-more-check-{target_suffix}", "confirm-more-check"),
                           (f"rare-tail-extension-main-{target_suffix}", "extension-main"),
                           (f"rare-tail-extension-check-{target_suffix}", "extension-check"),
                           (f"rare-tail-reuse-pilot-{target_suffix}", "reuse-pilot"),
                           (f"rare-tail-reuse-{target_suffix}", "reuse-training")):
            labels[derive(args.run_seed, tag)] = tag + f" [{label}]"
        pilot = derive(args.run_seed, f"rare-tail-pilot-{target_suffix}")
        for tag in ("lazy", "multimode", "fixture-bias", "cardinality-cases"):
            labels[derive(pilot, tag)] = f"rare-tail-pilot-{target_suffix} -> {tag}"
        for stream in streams:
            stream["source_label"] = labels.get(stream["stream_seed"], "unmapped derived stream")
    result = aggregate(streams)
    games = {g["id"]: g for g in request["games"]}
    team_ids = [t["team_id"] for t in request["team_groups"]]
    rules = request["phase"]["championship"]
    base = {t["team_id"]: {"points": t.get("add_sub", 0), "wins": 0, "gd": 0, "gf": 0, "ga": 0}
            for t in request["team_groups"]}
    def add_score(campaigns, game, hs, aws):
        for team, own, other in ((game["home_id"], hs, aws), (game["away_id"], aws, hs)):
            c = campaigns[team]
            c["gf"] += own; c["ga"] += other
            if own > other:
                c["wins"] += 1; c["points"] += rules["point_win"]
            elif own == other:
                c["points"] += rules["point_draw"]
            else:
                c["points"] += rules["point_loss"]
    for game in request["games"]:
        if game["played"]:
            add_score(base, game, game["home_score"], game["away_score"])
    for stream in result["streams"]:
        for donor in stream["top_32_weighted_hits"]:
            campaigns = {team: dict(c) for team, c in base.items()}
            fixture_codes = iter(donor["outcomes"])
            score_lookup = {row["game_id"]: (row["home_score"], row["away_score"])
                            for row in donor.get("scores") or []}
            for game in request["games"]:
                if game["played"]:
                    continue
                if donor.get("score_sampled") is True:
                    hs, aws = score_lookup[game["id"]]
                else:
                    code = next(fixture_codes)
                    hs, aws = (0, 1) if code == 0 else (0, 0) if code == 1 else (1, 0)
                add_score(campaigns, game, hs, aws)
            for c in campaigns.values():
                c["gd"] = c["gf"] - c["ga"]
            if donor.get("score_sampled") is not True:
                for c in campaigns.values():
                    c["gd"] = c["gf"] = c["ga"] = None
            donor["actual_campaigns"] = {str(team): campaigns[team] for team in team_ids}
            donor["observed_score_wdl_agrees_with_outcome"] = None
            if donor.get("score_sampled") is True:
                score_outcomes = []
                ordered_scores = sorted(donor["scores"], key=lambda x: next(
                    i for i, g in enumerate(request["games"]) if g["id"] == x["game_id"]))
                for row in ordered_scores:
                    score_outcomes.append(2 if row["home_score"] > row["away_score"] else
                                          1 if row["home_score"] == row["away_score"] else 0)
                donor["observed_score_wdl_agrees_with_outcome"] = score_outcomes == donor["outcomes"]
    for stream in result["streams"]:
        for row in stream["fixture_wdl_weighted_marginals"]:
            game = games[row["game_id"]]
            row["description"] = f"{row['home_id']} vs {row['away_id']}"
            row["played"] = game["played"]
    if args.output:
        args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    else:
        print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
