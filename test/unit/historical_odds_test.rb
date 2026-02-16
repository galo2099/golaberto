require 'test_helper'

class HistoricalOddsTest < ActiveSupport::TestCase
  test "calculate_odds returns sum of probabilities for given positions" do
    ho = HistoricalOdds.new(odds: [0.1, 0.2, 0.3, 0.4])
    assert_equal 0.3, ho.calculate_odds([1, 2])
    assert_equal 0.7, ho.calculate_odds([3, 4])
    assert_equal 0.6, ho.calculate_odds([1, 2, 3])
  end

  test "calculate_odds returns nil if odds are missing" do
    ho = HistoricalOdds.new(odds: nil)
    assert_nil ho.calculate_odds([1, 2])
  end

  test "calculate_odds returns 0 if no positions match" do
    ho = HistoricalOdds.new(odds: [0.1, 0.2])
    assert_equal 0, ho.calculate_odds([])
  end
end
