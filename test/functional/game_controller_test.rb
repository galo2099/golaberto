require File.dirname(__FILE__) + '/../test_helper'
require 'game_controller'

# Re-raise errors caught by the controller.
class GameController; def rescue_action(e) raise e end; end

class GameControllerTest < ActionController::TestCase
  tests GameController

  def setup
    @controller = GameController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
    @championship = Championship.create!(
      name: "Campeonato Teste",
      begin: Date.new(2020, 1, 1),
      end: Date.new(2020, 12, 31),
      point_win: 3,
      point_draw: 1,
      point_loss: 0,
      category: categories(:one),
      region: :national,
      region_name: "National"
    )
    @phase = @championship.phases.create!(
      name: "Fase Unica",
      order_by: 1,
      sort: "pt"
    )
    @game = @phase.games.create!(
      home: teams(:first),
      away: teams(:another),
      date: Time.zone.parse("2020-01-10 12:00:00"),
      played: false,
      home_score: 0,
      away_score: 0
    )
  end

  def test_show_links_team_names_to_championship_team_view
    get :show, params: { id: @game.to_param }

    assert_response :success
    assert_select "td.game_show_home_team a[href=?]", url_for(controller: :championship, action: :team, id: @championship, team: teams(:first))
    assert_select "td.game_show_away_team a[href=?]", url_for(controller: :championship, action: :team, id: @championship, team: teams(:another))
  end
end
