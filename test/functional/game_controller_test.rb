require File.dirname(__FILE__) + '/../test_helper'
require 'game_controller'

# Re-raise errors caught by the controller.
class GameController; def rescue_action(e) raise e end; end

class GameControllerTest < Test::Unit::TestCase
  def setup
    @controller = GameController.new
    @request    = ActionController::TestRequest.new
    @response   = ActionController::TestResponse.new
  end

  def test_default_games_page_for_scheduled_lists_page_with_todays_games
    today = Date.new(2026, 3, 30)
    phase = build_phase_for_category(categories(:one))
    @controller.stub(:cookie_timezone, ActiveSupport::TimeZone["UTC"]) do
      30.times do |i|
        Game.create!(:phase => phase, :home => teams(:first), :away => teams(:another), :date => (today - 31 + i).beginning_of_day, :played => false, :championship_category_id => 1)
      end
      Game.create!(:phase => phase, :home => teams(:first), :away => teams(:another), :date => today.beginning_of_day, :played => false, :championship_category_id => 1)

      page = @controller.send(:default_games_page, Game.where(:played => false, :championship_category_id => 1), :asc, 30)
      assert_equal 2, page
    end
  end

  def test_default_games_page_for_played_lists_page_with_todays_games
    today = Date.new(2026, 3, 30)
    phase = build_phase_for_category(categories(:one))
    @controller.stub(:cookie_timezone, ActiveSupport::TimeZone["UTC"]) do
      30.times do |i|
        Game.create!(:phase => phase, :home => teams(:first), :away => teams(:another), :date => (today + 1 + i).beginning_of_day, :played => true, :championship_category_id => 1)
      end
      Game.create!(:phase => phase, :home => teams(:first), :away => teams(:another), :date => today.beginning_of_day, :played => true, :championship_category_id => 1)

      page = @controller.send(:default_games_page, Game.where(:played => true, :championship_category_id => 1), :desc, 30)
      assert_equal 2, page
    end
  end

  private

  def build_phase_for_category(category)
    championship = Championship.create!(
      :name => "Page calc championship",
      :begin => Date.new(2026, 1, 1),
      :end => Date.new(2026, 12, 31),
      :point_win => 3,
      :point_draw => 1,
      :point_loss => 0,
      :category => category
    )
    championship.phases.create!(:name => "Phase", :order_by => rand(1000) + 10, :sort => "points")
  end
end
