require File.dirname(__FILE__) + '/../test_helper'

class ChampionshipTest < Test::Unit::TestCase
  def test_clone_phase_to_new_championship_copies_phase_groups_and_teams_without_games_or_players
    category = Category.find(1)
    source_championship = Championship.create!(
      name: "Source Championship",
      begin: Date.new(2025, 1, 1),
      end: Date.new(2025, 12, 31),
      point_win: 3,
      point_draw: 1,
      point_loss: 0,
      category: category,
      region: :national,
      region_name: "Brazil"
    )

    phase = source_championship.phases.create!(
      name: "Phase A",
      order_by: 1,
      sort: "pt,gd",
      bonus_points: 2,
      bonus_points_threshold: 5
    )

    group = phase.groups.create!(
      name: "Group A",
      zones: [{ "name" => "Promotion", "color" => "#00ff00", "position" => [1] }]
    )

    team_one = Team.find(1)
    team_two = Team.find(2)
    group.team_groups.create!(team: team_one, add_sub: 1, bias: 2, comment: "favored")
    group.team_groups.create!(team: team_two, add_sub: -1, bias: -2, comment: "underdog")

    phase.games.create!(home: team_one, away: team_two, played: true, date: Time.zone.now, home_score: 1, away_score: 0)
    TeamPlayer.create!(championship: source_championship, team: team_one, player: Player.create!(name: "Player One", country: "Brazil"))

    cloned_championship = source_championship.clone_phase_to_new_championship!(phase)
    cloned_phase = cloned_championship.phases.first
    cloned_group = cloned_phase.groups.first

    assert_equal "Source Championship - Phase A", cloned_championship.name
    assert_equal source_championship.begin, cloned_championship.begin
    assert_equal source_championship.end, cloned_championship.end
    assert_equal 1, cloned_championship.phases.count
    assert_equal "Phase A", cloned_phase.name
    assert_equal "pt,gd", cloned_phase.sort
    assert_equal 2, cloned_phase.bonus_points
    assert_equal 5, cloned_phase.bonus_points_threshold
    assert_equal 1, cloned_phase.groups.count
    assert_equal "Group A", cloned_group.name
    assert_equal group.zones, cloned_group.zones
    assert_equal [team_one.id, team_two.id].sort, cloned_group.team_groups.pluck(:team_id).sort
    assert_equal 0, cloned_phase.games.count
    assert_equal 0, TeamPlayer.where(championship_id: cloned_championship.id).count
  end
end
