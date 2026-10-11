require File.dirname(__FILE__) + '/../test_helper'

class TeamGroupTest < Test::Unit::TestCase
  def test_zone_with_no_odds_outside_it_is_exactly_one_hundred
    team_group = TeamGroup.new(odds: [77.75802002807019, 17.064525344641567, 5.177454627288232])

    assert_equal 100.0, team_group.calculate_odds([1, 2, 3])

    team_group.odds = [99.9999999999, 0, 0]
    team_group.odds_reachability = %w[reachable impossible impossible]
    assert_equal 100.0, team_group.calculate_odds([1, 2])
    assert_equal 100.0, TeamGroup.calculate_odds_for(team_group.odds, [1, 2], team_group.odds_reachability)
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
    team_group.odds = [100.0, 0.0]
    team_group.odds_reachability = %w[reachable impossible]
    captured_at = Time.zone.parse("2026-10-11 10:00")
    team_group.record_odds_snapshot!(captured_at)

    snapshot = team_group.odds_histories.find_by!(recorded_on: captured_at.to_date)
    assert_equal %w[reachable impossible], snapshot.odds_reachability

    team_group.odds_reachability = nil
    team_group.record_odds_snapshot!(captured_at + 1.hour)

    snapshot.reload
    assert_nil snapshot.odds_reachability
    assert_equal [100.0, 0.0], snapshot.odds
  end
end
