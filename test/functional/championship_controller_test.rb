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
end
