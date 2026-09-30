require 'minitest/autorun'
require 'action_view'
require_relative '../../app/helpers/odds_formatting_helper'

class OddsFormattingTest < Minitest::Test
  def setup
    @view = Object.new
    @view.extend(ActionView::Helpers::NumberHelper)
    @view.extend(ActionView::Helpers::TagHelper)
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

  def test_compact_team_odds_examples_and_rounding
    examples = {
      99.98 => '99.98',
      99.9999999321 => '99.[07]3',
      99.9999999789 => '99.[07]8',
      99.9999994 => '99.[06]4',
      99.999999905 => '99.[07]1',
      99.9999999049 => '99.[06]9',
      99.995 => '99.[02]5',
      42.35453 => '42.35',
      0.0000004 => '0.[06]4',
      0.0000000000876 => '0.[10]9',
      12 => '12.00',
      100 => '100.0',
      0 => '0.0',
      1.005 => '1.01',
      0.005 => '0.01',
      0.00000045 => '0.[06]5',
      0.00000095 => '0.[05]1',
      99.99945 => '99.[03]5',
      99.999449999 => '99.[03]4',
      99.99999999999999 => '99.[13]9'
    }
    examples.each do |value, expected|
      assert_equal expected, @view.compact_team_odds_text(value), value.to_s
    end
    assert_equal '', @view.compact_team_odds_text(nil)
    assert_equal '0.[99]1', @view.compact_team_odds_text(BigDecimal('1e-100'))
    assert_equal '1e-101', @view.compact_team_odds_text(BigDecimal('1e-101'))
    assert_equal '100 − 1e-101', @view.compact_team_odds_text(BigDecimal('100') - BigDecimal('1e-101'))
  end

  def test_compact_team_odds_markup_and_exact_title
    markup = @view.formatted_compact_team_odds(0.0000004)
    assert_includes markup, 'odds-compact-integer'
    assert_includes markup, 'odds-compact-count'
    assert_includes markup, '[06]'
    assert_equal ['6'], Nokogiri::HTML.fragment(markup).css('.odds-compact-count-digit').map(&:text)
    assert_equal '..', Nokogiri::HTML.fragment(markup).at_css('.odds-compact-count-dots').text
    double_digit = @view.formatted_compact_team_odds(0.0000000000876)
    assert_equal ['10'], Nokogiri::HTML.fragment(double_digit).css('.odds-compact-count-digit').map(&:text)
    assert_equal "#{0.0000004}%", @view.compact_team_odds_title(0.0000004)
  end

  def test_odds_background_blends_from_lightgray_to_dimgray
    assert_equal 'lightgray', @view.odds_background_color(0, 25)
    assert_equal 'lightgray', @view.odds_background_color(nil, 25)
    assert_equal 'lightgray', @view.odds_background_color(25, 0)
    assert_equal 'rgb(136, 136, 136)', @view.odds_background_color(12.5, 25)
    assert_equal 'rgb(167, 167, 167)', @view.odds_background_color(10, 57)
    assert_equal 'rgb(211, 211, 211)', @view.odds_background_color(1e-10, 25)
    assert_equal 'dimgray', @view.odds_background_color(25, 25)
  end

  def test_odds_text_switches_to_white_on_darker_gray
    assert_equal 'inherit', @view.odds_text_color(0, 25)
    assert_equal 'inherit', @view.odds_text_color(10, 25)
    assert_equal 'white', @view.odds_text_color(11, 25)
    assert_equal 'white', @view.odds_text_color(25, 25)
  end
end
