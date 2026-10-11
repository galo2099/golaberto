require File.dirname(__FILE__) + '/../test_helper'
require File.expand_path('../../db/migrate/20261011120000_compact_team_group_odds_histories', __dir__)

class CompactTeamGroupOddsHistoriesMigrationTest < ActiveSupport::TestCase
  test "compacts legacy odds without changing values or timestamps" do
    team_group = team_groups(:first)
    captured_at = Time.zone.parse("2024-01-01 10:00")
    snapshot = team_group.odds_histories.create!(
      recorded_on: captured_at.to_date,
      captured_at: captured_at,
      odds: [99.0, 0.0, [75.0], [0.0], [0.0, "impossible"], [0.0, "reachable"], [0.0, "undecided"], [0.0, "unknown"]],
      created_at: captured_at - 2.days,
      updated_at: captured_at - 1.day
    )
    original_created_at = snapshot.created_at
    original_updated_at = snapshot.updated_at

    CompactTeamGroupOddsHistories.new.up

    snapshot.reload
    assert_equal [99.0, 0.0, 75.0, 0.0, [0.0, "impossible"], [0.0, "reachable"], 0.0, 0.0], snapshot.odds
    assert_equal [99.0, 0.0, 75.0, 0.0, 0.0, 0.0, 0.0, 0.0], snapshot.numeric_odds
    assert_equal %w[reachable undecided reachable undecided impossible reachable undecided undecided], snapshot.odds_reachability
    assert_equal captured_at, snapshot.captured_at
    assert_equal captured_at.to_date, snapshot.recorded_on
    assert_equal original_created_at, snapshot.created_at
    assert_equal original_updated_at, snapshot.updated_at

    CompactTeamGroupOddsHistories.new.up
    snapshot.reload
    assert_equal [99.0, 0.0, 75.0, 0.0, [0.0, "impossible"], [0.0, "reachable"], 0.0, 0.0], snapshot.odds
    assert_equal original_updated_at, snapshot.updated_at
  end

  test "migration cannot be reversed without losing the distinction for reachable zeroes" do
    error = assert_raises(ActiveRecord::IrreversibleMigration) do
      CompactTeamGroupOddsHistories.new.down
    end

    assert_match(/would discard reachability proofs/, error.message)
  end

  test "a failed migration can resume after earlier rows were converted" do
    team_group = team_groups(:first)
    captured_at = Time.zone.parse("2024-02-01 10:00")
    positive_singleton = team_group.odds_histories.create!(
      recorded_on: captured_at.to_date,
      captured_at: captured_at,
      odds: [[73.5]]
    )
    undecided = team_group.odds_histories.create!(
      recorded_on: captured_at.to_date + 1,
      captured_at: captured_at + 1.day,
      odds: [[0.0, "undecided"]]
    )
    rows = [
      CompactTeamGroupOddsHistories::Snapshot.find(positive_singleton.id),
      CompactTeamGroupOddsHistories::Snapshot.find(undecided.id),
    ]
    rows.last.define_singleton_method(:update_columns) { |*| raise "simulated write failure" }

    CompactTeamGroupOddsHistories::Snapshot.stub(:find_each, ->(batch_size:, &block) { rows.each(&block) }) do
      assert_raises(RuntimeError) { CompactTeamGroupOddsHistories.new.up }
    end

    assert_equal [73.5], positive_singleton.reload.odds
    assert_equal [[0.0, "undecided"]], undecided.reload.odds

    CompactTeamGroupOddsHistories.new.up
    assert_equal [73.5], positive_singleton.reload.odds
    assert_equal [0.0], undecided.reload.odds
  end
end
