require File.dirname(__FILE__) + '/../test_helper'

class PhaseTest < Test::Unit::TestCase
  def test_parsed_sofascore_tournament_ids_accepts_spaces_and_commas
    phase = Phase.new(sofascore_tournament_ids: "27214, 90333\n27214 invalid 27214x")

    assert_equal [27214, 90333], phase.parsed_sofascore_tournament_ids
  end

  def test_parsed_sofascore_tournament_ids_is_empty_without_a_filter
    assert_empty Phase.new.parsed_sofascore_tournament_ids
  end
end
