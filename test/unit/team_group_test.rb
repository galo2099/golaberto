require File.dirname(__FILE__) + '/../test_helper'

class TeamGroupTest < Test::Unit::TestCase
  def test_zone_with_no_odds_outside_it_is_exactly_one_hundred
    team_group = TeamGroup.new(odds: [77.75802002807019, 17.064525344641567, 5.177454627288232])

    assert_equal 100.0, team_group.calculate_odds([1, 2, 3])

    team_group.odds = [99.9999999999, 0, 0]
    assert_equal 100.0, team_group.calculate_odds([1, 2])
    assert_equal 100.0, TeamGroup.calculate_odds_for(team_group.odds, [1, 2])
  end

  def test_zone_keeps_a_nonzero_residual_outside_it
    team_group = TeamGroup.new(odds: [99.9999999, 0, 0.0000001])

    assert_equal 99.9999999, team_group.calculate_odds([1, 2])

    team_group.odds = [99.9999999999, nil]
    assert_equal 99.9999999999, team_group.calculate_odds([1])

    team_group.odds = [99.9999999999, nil, 0]
    assert_equal 99.9999999999, team_group.calculate_odds([1, 2])
  end
end
