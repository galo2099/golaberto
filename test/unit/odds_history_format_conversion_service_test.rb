require File.dirname(__FILE__) + '/../test_helper'

class OddsHistoryFormatConversionServiceTest < ActiveSupport::TestCase
  test "converts legacy and partial metadata rows while preserving timestamps and odds" do
    team_group = team_groups(:first)
    first_time = Time.zone.parse("2024-01-01 10:00")
    second_time = Time.zone.parse("2024-01-02 10:00")
    canonical_time = Time.zone.parse("2024-01-03 10:00")
    legacy = team_group.odds_histories.create!(
      recorded_on: first_time.to_date,
      captured_at: first_time,
      odds: [0.0, [73.5], 26.5, [0.0], [0.0, "impossible"], [0.0, "reachable"], [0.0, "invalid"]],
      created_at: first_time,
      updated_at: first_time
    )
    partial_metadata = team_group.odds_histories.create!(
      recorded_on: second_time.to_date,
      captured_at: second_time,
      odds: [[0.0, "reachable"], [0.0]],
      created_at: second_time,
      updated_at: second_time
    )
    canonical_odds = [65.0, [0.0, "impossible"], [0.0, "reachable"], [0.0, "undecided"]]
    canonical = team_group.odds_histories.create!(
      recorded_on: canonical_time.to_date,
      captured_at: canonical_time,
      odds: canonical_odds,
      created_at: canonical_time,
      updated_at: canonical_time
    )

    assert_equal 2, OddsHistoryFormatConversionService.run(batch_size: 2)

    legacy.reload
    assert_equal [[0.0, "undecided"], 73.5, 26.5, [0.0, "undecided"], [0.0, "impossible"], [0.0, "reachable"], [0.0, "undecided"]], legacy.odds
    assert_equal [0.0, 73.5, 26.5, 0.0, 0.0, 0.0, 0.0], legacy.numeric_odds
    assert_equal %w[undecided reachable reachable undecided impossible reachable undecided], legacy.odds_reachability
    assert_equal first_time, legacy.created_at
    assert_equal first_time, legacy.updated_at
    assert_equal first_time, legacy.captured_at
    assert_equal first_time.to_date, legacy.recorded_on

    partial_metadata.reload
    assert_equal [[0.0, "reachable"], [0.0, "undecided"]], partial_metadata.odds
    assert_equal [0.0, 0.0], partial_metadata.numeric_odds
    assert_equal %w[reachable undecided], partial_metadata.odds_reachability
    assert_equal second_time, partial_metadata.updated_at

    canonical.reload
    assert_equal canonical_odds, canonical.odds
    assert_equal canonical_time, canonical.updated_at
    assert_equal 0, OddsHistoryFormatConversionService.run(batch_size: 2)
  end
end
