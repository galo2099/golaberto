require 'minitest/autorun'
require 'action_view'
require_relative '../../app/helpers/odds_formatting_helper'

class OddsFormattingTest < Minitest::Test
  def setup
    @view = Object.new
    @view.extend(ActionView::Helpers::NumberHelper)
    @view.extend(OddsFormattingHelper)
  end

  def test_small_nonzero_odds_remain_visible_and_recoverable
    assert_equal '<0.01%', @view.formatted_odds(0.0001)
    assert_equal '1e-4%', @view.odds_title(0.0001)
    assert_equal '4.32e-5%', @view.odds_title(0.00004321)
    assert_equal '1e-2%', @view.odds_title(0.009999)
  end

  def test_near_certain_odds_do_not_round_to_certain
    assert_equal '>99.99%', @view.formatted_odds(99.9999)
    assert_equal '99.9999%', @view.odds_title(99.9999)
    assert_equal '99.9%', @view.odds_title(99.95)
  end

  def test_ordinary_titles_have_at_most_three_significant_digits
    assert_equal '0.0123%', @view.odds_title(0.012345)
    assert_equal '12.3%', @view.odds_title(12.3456)
  end

  def test_exact_endpoints_and_missing_odds
    assert_equal '0.00%', @view.formatted_odds(0)
    assert_equal '0.01%', @view.formatted_odds(0.01)
    assert_equal '99.99%', @view.formatted_odds(99.99)
    assert_equal '100.00%', @view.formatted_odds(100)
    assert_equal '', @view.formatted_odds(nil)
    assert_nil @view.odds_title(nil)
  end

  def test_very_small_title_does_not_round_to_zero
    assert_equal '1e-15%', @view.odds_title(1e-15)
    refute_equal '100%', @view.odds_title(99.99999999999999)
  end
end
