require File.dirname(__FILE__) + '/../test_helper'
require 'championship_controller'

# Re-raise errors caught by the controller.
class ChampionshipController; def rescue_action(e) raise e end; end

class ChampionshipControllerTest < Test::Unit::TestCase
  def setup
    @controller = ChampionshipController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  # Replace this with your real tests.
  def test_truth
    assert true
  end

  def test_generate_odds_history_json_tracks_matches_played_with_uniform_match_steps
    championship = Championship.create!(
      name: "Spacing Championship",
      begin: Date.new(2025, 1, 1),
      end: Date.new(2025, 12, 31),
      point_win: 3,
      point_draw: 1,
      point_loss: 0,
      region: :national,
      region_name: "Brazil",
      category: categories(:one)
    )
    phase = Phase.create!(
      championship: championship,
      name: "Spacing Phase",
      order_by: 1,
      sort: "pt,gd"
    )
    group = Group.create!(
      phase: phase,
      name: "Spacing Group",
      zones: []
    )

    team = teams(:first)
    opponent = teams(:another)

    team_group = TeamGroup.create!(group: group, team: team, add_sub: 0, bias: 0)
    TeamGroup.create!(group: group, team: opponent, add_sub: 0, bias: 0)

    Game.create!(
      phase: phase,
      home: team,
      away: opponent,
      date: Time.zone.parse("2025-01-02 12:00:00"),
      played: true,
      home_score: 1,
      away_score: 0
    )
    Game.create!(
      phase: phase,
      home: opponent,
      away: team,
      date: Time.zone.parse("2025-01-06 12:00:00"),
      played: true,
      home_score: 0,
      away_score: 2
    )

    TeamGroupOddsHistory.create!(
      team_group: team_group,
      recorded_on: Date.new(2025, 1, 1),
      captured_at: Time.zone.parse("2025-01-01 23:59:00"),
      odds: [60.0, 40.0]
    )
    TeamGroupOddsHistory.create!(
      team_group: team_group,
      recorded_on: Date.new(2025, 1, 3),
      captured_at: Time.zone.parse("2025-01-03 23:59:00"),
      odds: [55.0, 45.0]
    )
    TeamGroupOddsHistory.create!(
      team_group: team_group,
      recorded_on: Date.new(2025, 1, 5),
      captured_at: Time.zone.parse("2025-01-05 23:59:00"),
      odds: [50.0, 50.0]
    )
    TeamGroupOddsHistory.create!(
      team_group: team_group,
      recorded_on: Date.new(2025, 1, 7),
      captured_at: Time.zone.parse("2025-01-07 23:59:00"),
      odds: [65.0, 35.0]
    )

    payload = JSON.parse(@controller.send(:generate_odds_history_json, group, team))
    meta = payload["series"].first["point_meta"]

    assert_equal [0, 1, 1, 2], meta.map { |point_meta| point_meta["matches_played"] }
  end
end
