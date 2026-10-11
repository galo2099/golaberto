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

  def test_player_list_order_clause_adds_stable_tie_breakers_for_desc
    assert_equal(
      "goals DESC, players.name ASC, players.id ASC",
      @controller.send(:player_list_order_clause, 4, "DESC")
    )
  end

  def test_player_list_order_clause_adds_stable_tie_breakers_for_asc
    assert_equal(
      "players.name ASC, players.name ASC, players.id ASC",
      @controller.send(:player_list_order_clause, 0, "ASC")
    )
  end

  def test_player_list_footer_formats_totals
    totals = {
      "minutes" => "180",
      "goals" => "4",
      "own_goals" => "1",
      "penalties" => "2",
      "appearances" => "7",
      "played" => "6",
      "sub" => "3",
      "bench" => "1",
      "yellows" => "2",
      "reds" => "1",
      "off_rating" => "5.5",
      "def_rating" => "4.0",
    }
    @controller.define_singleton_method(:relation_totals) { |_relation| totals }

    footer = @controller.send(:player_list_footer, Object.new)

    assert_equal "Total", footer[0]
    assert_equal 180, footer[3]
    assert_equal 4, footer[4]
    assert_equal "2.00", footer[5]
    assert_equal "9.50", footer[6]
    assert_equal "4.75", footer[7]
    assert_equal "5.50", footer[8]
    assert_equal "4.00", footer[9]
    assert_equal 1, footer[10]
    assert_equal 2, footer[11]
    assert_equal 7, footer[12]
    assert_equal 6, footer[13]
    assert_equal 3, footer[14]
    assert_equal 1, footer[15]
    assert_equal 2, footer[16]
    assert_equal 1, footer[17]
  end

  def test_odds_history_json_includes_snapshot_reachability_for_points_and_zones
    snapshot = TeamGroupOddsHistory.new(
      recorded_on: Date.new(2026, 10, 11),
      captured_at: Time.zone.parse("2026-10-11 10:00"),
      odds: [100.0, [0.0, "impossible"], [0.0, "reachable"], [0.0, "undecided"]]
    )
    legacy_snapshot = TeamGroupOddsHistory.new(
      recorded_on: Date.new(2026, 10, 10),
      captured_at: Time.zone.parse("2026-10-10 10:00"),
      odds: [100.0, 0.0, 0.0, 0.0]
    )
    histories = [legacy_snapshot, snapshot]
    histories.define_singleton_method(:order) { |*| self }
    team_group = Struct.new(:odds_histories).new(histories)
    team_groups = Object.new
    team_groups.define_singleton_method(:size) { 4 }
    team_groups.define_singleton_method(:find_by) { |team_id:| team_group }
    games = Object.new
    games.define_singleton_method(:where) { |*| self }
    games.define_singleton_method(:includes) { |*| self }
    games.define_singleton_method(:order) { |*| self }
    games.define_singleton_method(:to_a) { [] }
    group = Struct.new(:zones, :team_groups, :games).new(
      [
        { "name" => "Impossible", "color" => "red", "position" => [2] },
        { "name" => "Reachable", "color" => "green", "position" => [2, 3] },
        { "name" => "Undecided", "color" => "yellow", "position" => [2, 4] },
      ], team_groups, games
    )
    json = JSON.parse(@controller.send(:generate_odds_history_json, group, Struct.new(:id).new(7)))

    series_by_label = json.fetch("series").index_by { |series| series.fetch("label") }
    assert_equal "reachable", series_by_label.fetch("1st").fetch("point_meta").last.fetch("reachability")
    assert_equal "impossible", series_by_label.fetch("2nd").fetch("point_meta").last.fetch("reachability")
    assert_equal "reachable", series_by_label.fetch("3rd").fetch("point_meta").last.fetch("reachability")
    assert_equal "undecided", series_by_label.fetch("4th").fetch("point_meta").last.fetch("reachability")
    assert_equal "reachable", series_by_label.fetch("1st").fetch("point_meta").first.fetch("reachability")
    assert_equal "undecided", series_by_label.fetch("2nd").fetch("point_meta").first.fetch("reachability")

    zone_odds = series_by_label.fetch("1st").fetch("point_meta").last.fetch("zone_odds")
    assert_equal %w[impossible reachable undecided], zone_odds.map { |zone| zone.fetch("reachability") }
  end
end
