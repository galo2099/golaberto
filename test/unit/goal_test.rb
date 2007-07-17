require File.dirname(__FILE__) + '/../test_helper'

class GoalTest < Test::Unit::TestCase
  fixtures :goals
  fixtures :players
  fixtures :games
  fixtures :teams

  # Replace this with your real tests.
  def test_penalty
    goal_attr = { :player_id => 1,
                  :team_id => 1,
                  :game_id => 1,
                  :time => 10,
                  :penalty => 0,
                  :own_goal => 0 }
    goal = Goal.new(goal_attr)
    assert goal.team
    assert goal.player
    assert goal.game
    assert goal.save!
  end
end
