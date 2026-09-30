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
    assert_equal '1e-4%', @view.formatted_odds(0.0001)
    assert_equal '4e-5%', @view.formatted_odds(0.00004321)
    assert_equal '1e-2%', @view.formatted_odds(0.009999)
    assert_equal '1e-4%', @view.odds_title(0.0001)
    assert_equal '4.321e-5%', @view.odds_title(0.00004321)
    assert_equal '9.999e-3%', @view.odds_title(0.009999)
  end

  def test_near_certain_odds_do_not_round_to_certain
    assert_equal '>99.99%', @view.formatted_odds(99.9999)
    assert_equal '>99.99%', @view.formatted_odds(99.99996)
    assert_equal '99.9999%', @view.odds_title(99.9999)
    assert_equal '99.95%', @view.odds_title(99.95)
  end

  def test_ordinary_titles_have_at_most_four_significant_digits
    assert_equal '0.01235%', @view.odds_title(0.012345)
    assert_equal '12.35%', @view.odds_title(12.3456)
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
    assert_equal '1e-15%', @view.formatted_odds(1e-15)
    assert_equal '1e-15%', @view.odds_title(1e-15)
    refute_equal '100%', @view.odds_title(99.99999999999999)
  end

  def test_standings_table_uses_fraction_scale
    assert_equal '', @view.formatted_odds_fraction(nil)
    assert_equal '.0000', @view.formatted_odds_fraction(0)
    assert_equal '.9800', @view.formatted_odds_fraction(98)
    assert_equal '.5000', @view.formatted_odds_fraction(50)
    assert_equal '.9975', @view.formatted_odds_fraction(99.75)
    assert_equal '.9999', @view.formatted_odds_fraction(99.99)
    assert_equal '1-3e-7', @view.formatted_odds_fraction(99.99997)
    assert_equal '.0001', @view.formatted_odds_fraction(0.01)
    assert_equal '4e-7', @view.formatted_odds_fraction(0.00004321)
    assert_equal '1.000', @view.formatted_odds_fraction(100)
  end

  def test_odds_background_blends_from_gray_to_zone_color
    assert_equal 'lightgray', @view.odds_background_color(0, '#90EE90', 25)
    assert_equal 'lightgray', @view.odds_background_color(nil, '#90EE90', 25)
    assert_equal 'lightgray', @view.odds_background_color(0, '#90EE90', 0)
    assert_equal 'rgb(164, 230, 164)', @view.odds_background_color(12.5, '#90EE90', 25)
    assert_equal 'rgb(183, 222, 183)', @view.odds_background_color(10, '#90EE90', 57)
    assert_equal 'rgb(211, 211, 211)', @view.odds_background_color(1e-10, '#90EE90', 25)
    assert_equal 'rgb(62, 242, 62)', @view.odds_background_color(12.5, 'rgb(0,255,0)', 25)
    assert_equal 'rgb(242, 62, 242)', @view.odds_background_color(12.5, '#f0f', 25)
    assert_equal '#90EE90', @view.odds_background_color(25, '#90EE90', 25)
    assert_equal 'lightgray', @view.odds_background_color(25, nil, 25)
    assert_equal 'lightgray', @view.odds_background_color(0, 'dimgray', 57)
    assert_equal 'rgb(167, 167, 167)', @view.odds_background_color(10, 'dimgray', 57)
    assert_equal 'dimgray', @view.odds_background_color(57, 'dimgray', 57)
  end

  def test_black_zone_text_remains_readable_during_blend
    assert_equal 'inherit', @view.odds_text_color(0, 'black', 25)
    assert_equal 'inherit', @view.odds_text_color(5, 'black', 25)
    assert_equal 'white', @view.odds_text_color(5.0625, 'black', 25)
    assert_equal 'white', @view.odds_text_color(25, 'black', 25)
    assert_equal 'inherit', @view.odds_text_color(10, 'dimgray', 25)
    assert_equal 'white', @view.odds_text_color(11, 'dimgray', 25)
    assert_equal '0.5px 0 #fff, -0.5px 0 #fff, 0 0.5px #fff, 0 -0.5px #fff', @view.odds_text_shadow(10, 'dimgray', 25)
    assert_equal '0.5px 0 #222, -0.5px 0 #222, 0 0.5px #222, 0 -0.5px #222', @view.odds_text_shadow(11, 'dimgray', 25)
  end
end
