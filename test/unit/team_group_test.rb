require File.dirname(__FILE__) + '/../test_helper'

class TeamGroupTest < Test::Unit::TestCase
  def test_zone_with_no_odds_outside_it_is_exactly_one_hundred
    team_group = TeamGroup.new(odds: [77.75802002807019, 17.064525344641567, 5.177454627288232])

    assert_equal 100.0, team_group.calculate_odds([1, 2, 3])

    team_group.odds = [99.9999999999, [0, "impossible"], [0, "impossible"]]
    assert_equal 100.0, team_group.calculate_odds([1, 2])
    assert_equal 100.0, TeamGroup.calculate_odds_for(team_group.numeric_odds, [1, 2], team_group.odds_reachability)
    assert_equal 100.0, TeamGroup.calculate_odds_for([99.99999999999062, 0, 0], [1], %w[reachable impossible impossible])
  end

  def test_zone_keeps_a_nonzero_residual_outside_it
    team_group = TeamGroup.new(odds: [99.9999999, 0, 0.0000001])

    assert_equal 99.9999999, team_group.calculate_odds([1, 2])

    team_group.odds = [99.9999999999, nil]
    assert_equal 99.9999999999, team_group.calculate_odds([1])

    team_group.odds = [99.9999999999, nil, 0]
    assert_equal 99.9999999999, team_group.calculate_odds([1, 2])
  end

  def test_reachability_marks_zero_odds_and_aggregates_zones
    odds = [100.0, 0.0, 0.0]
    statuses = %w[reachable impossible undecided]

    assert_equal "impossible", TeamGroup.reachability_for(odds, statuses, [2])
    assert_equal "undecided", TeamGroup.reachability_for(odds, statuses, [2, 3])
    assert_equal "reachable", TeamGroup.reachability_for(odds, statuses, [1, 2])
    assert_equal "undecided", TeamGroup.reachability_for(odds, nil, [2])
  end

  def test_record_odds_snapshot_persists_and_clears_reachability_on_same_day
    team_group = team_groups(:first)
    team_group.odds = [100.0, [0.0, "impossible"], [0.0, "reachable"], 0.0, 0.0]
    captured_at = Time.zone.parse("2026-10-11 10:00")
    team_group.record_odds_snapshot!(captured_at)

    snapshot = team_group.odds_histories.find_by!(recorded_on: captured_at.to_date)
    assert_equal [100.0, [0.0, "impossible"], [0.0, "reachable"], 0.0, 0.0], snapshot.reload.odds
    assert_equal team_group.numeric_odds, snapshot.numeric_odds
    assert_equal %w[reachable impossible reachable undecided undecided], snapshot.odds_reachability

    team_group.odds = [100.0, 0.0, 0.0, 0.0, 0.0]
    team_group.record_odds_snapshot!(captured_at + 1.hour)

    snapshot.reload
    assert_equal [100.0, 0.0, 0.0, 0.0, 0.0], snapshot.odds
    assert_equal %w[reachable undecided undecided undecided undecided], snapshot.odds_reachability
  end

  def test_response_assignment_encodes_statuses_inline_and_clears_missing_proofs
    team_group = TeamGroup.new(team_id: 42)
    response = {
      "team_odds" => { "42" => { "Pos" => [0.0, 0.0, 0.0, 100.0] } },
      "rare_position_estimates" => { "42" => {
        "0" => { "reachability" => "impossible" },
        "1" => { "reachability" => "reachable" },
        "2" => { "reachability" => "undecided" },
        "3" => { "reachability" => "reachable" },
      } },
    }
    team_group.assign_calculated_odds!(response)

    assert_equal [[0.0, "impossible"], [0.0, "reachable"], 0.0, 100.0], team_group.odds
    assert_equal [0.0, 0.0, 0.0, 100.0], team_group.numeric_odds
    assert_equal %w[impossible reachable undecided reachable], team_group.odds_reachability

    team_group.assign_calculated_odds!(response.except("rare_position_estimates"))

    assert_equal [0.0, 0.0, 0.0, 100.0], team_group.odds
    assert_equal %w[undecided undecided undecided reachable], team_group.odds_reachability
  end

  def test_legacy_plain_odds_decode_without_reachability_proofs
    snapshot = TeamGroupOddsHistory.new(odds: [75.0, [0.0]])

    assert_equal [75.0, 0.0], snapshot.numeric_odds
    assert_equal %w[reachable undecided], snapshot.odds_reachability

    singleton_positive_legacy_snapshot = TeamGroupOddsHistory.new(odds: [[75.0], [0.0]])
    assert_equal [75.0, 0.0], singleton_positive_legacy_snapshot.numeric_odds
    assert_equal %w[reachable undecided], singleton_positive_legacy_snapshot.odds_reachability

    prior_reachable_zero_snapshot = TeamGroupOddsHistory.new(odds: [[0.0, "reachable"]])
    assert_equal [0.0], prior_reachable_zero_snapshot.numeric_odds
    assert_equal ["reachable"], prior_reachable_zero_snapshot.odds_reachability

    current_undecided_zero_snapshot = TeamGroupOddsHistory.new(odds: [0.0])
    assert_equal [0.0], current_undecided_zero_snapshot.numeric_odds
    assert_equal ["undecided"], current_undecided_zero_snapshot.odds_reachability
  end
end
