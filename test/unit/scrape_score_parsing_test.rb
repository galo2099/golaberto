require "test_helper"
require "scrape"

class ScrapeScoreParsingTest < ActiveSupport::TestCase
  test "extract_sofascore_scores prefers full-time score and captures extra-time goals" do
    home_score = { "current" => 3, "display" => 3, "normaltime" => 2, "afterExtraTime" => 3 }
    away_score = { "current" => 2, "display" => 2, "normaltime" => 2, "afterExtraTime" => 2 }

    scores = extract_sofascore_scores(home_score, away_score)

    assert_equal 2, scores[:full_time_home]
    assert_equal 2, scores[:full_time_away]
    assert_equal 1, scores[:aet_home]
    assert_equal 0, scores[:aet_away]
    assert_equal 3, scores[:final_home]
    assert_equal 2, scores[:final_away]
  end

  test "extract_sofascore_scores captures penalties when present" do
    home_score = { "current" => 1, "display" => 1, "normaltime" => 1, "penalties" => 4 }
    away_score = { "current" => 1, "display" => 1, "normaltime" => 1, "penalties" => 3 }

    scores = extract_sofascore_scores(home_score, away_score)

    assert_equal 4, scores[:pen_home]
    assert_equal 3, scores[:pen_away]
  end

  test "extract_sofascore_scores falls back to final score when full-time is missing" do
    home_score = { "display" => "2" }
    away_score = { "display" => "1" }

    scores = extract_sofascore_scores(home_score, away_score)

    assert_nil scores[:full_time_home]
    assert_nil scores[:full_time_away]
    assert_equal 2, scores[:final_home]
    assert_equal 1, scores[:final_away]
    assert_nil scores[:aet_home]
    assert_nil scores[:aet_away]
  end

  test "extract_sofascore_scores handles overtime as extra-time-only goals" do
    home_score = { "current" => 0, "display" => 0, "normaltime" => 0, "overtime" => 0, "extra1" => 0, "extra2" => 0 }
    away_score = { "current" => 2, "display" => 2, "normaltime" => 1, "overtime" => 1, "extra1" => 1, "extra2" => 0 }

    scores = extract_sofascore_scores(home_score, away_score)

    assert_equal 0, scores[:aet_home]
    assert_equal 1, scores[:aet_away]
  end
end
