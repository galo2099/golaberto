require File.dirname(__FILE__) + '/../test_helper'

class GoalTest < Test::Unit::TestCase

  def setup
    @goal_attr = { :player_id => 1,
                   :team_id => 1,
                   :game_id => 1,
                   :time => 10,
                   :penalty => 0,
                   :own_goal => 0 }
  end

  def test_crud
    goal = Goal.new(@goal_attr)
    assert goal.save

    g = Goal.find(goal.id)
    assert_equal g.time, goal.time

    g.time = 20
    assert g.save

    assert g.destroy
  end

  def test_validation
    goal = Goal.new(@goal_attr)
    goal.player_id = 0
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.game_id = 0
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.team_id = 0
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.time = nil
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.time = "foo"
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.penalty = nil
    assert !goal.save

    goal = Goal.new(@goal_attr)
    goal.own_goal = nil
    assert !goal.save
  end

  def test_penalty_boolean
    goal = Goal.new(@goal_attr)
    # Number 1 and string "1" are true
    goal.penalty = 1
    assert_equal true, goal.penalty
    goal.penalty = "1"
    assert_same true, goal.penalty
    goal.penalty = true
    assert_equal true, goal.penalty

    # Everything else is false
    goal.penalty = 0
    assert_equal false, goal.penalty
    goal.penalty = 2
    assert_equal false, goal.penalty

    goal.penalty = "0"
    assert_equal false, goal.penalty
    goal.penalty = "2"
    assert_equal false, goal.penalty

    goal.penalty = false
    assert_equal false, goal.penalty
  end

  def test_own_goal_boolean
    goal = Goal.new(@goal_attr)
    # Number 1 and string "1" are true
    goal.own_goal = 1
    assert_equal true, goal.own_goal
    goal.own_goal = "1"
    assert_same true, goal.own_goal
    goal.own_goal = true
    assert_equal true, goal.own_goal

    # Everything else is false
    goal.own_goal = 0
    assert_equal false, goal.own_goal
    goal.own_goal = 2
    assert_equal false, goal.own_goal

    goal.own_goal = "0"
    assert_equal false, goal.own_goal
    goal.own_goal = "2"
    assert_equal false, goal.own_goal

    goal.own_goal = false
    assert_equal false, goal.own_goal
  end
end
