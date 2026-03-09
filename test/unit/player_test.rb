require File.dirname(__FILE__) + '/../test_helper'

class PlayerTest < Test::Unit::TestCase
  def test_merge_player_repoints_references_and_keeps_target_conflicts
    target = Player.create!(name: 'Target', full_name: 'Ana', position: 'cm', birth: Date.new(1990, 1, 1), country: 'Brazil')
    source = Player.create!(name: 'Source', full_name: 'Ana Maria', position: 'fw', birth: Date.new(1992, 2, 2), country: 'Argentina')

    goal = Goal.create!(player_id: source.id, game_id: games(:first).id, team_id: teams(:first).id, time: 10, penalty: false, own_goal: false)
    source_player_game = PlayerGame.create!(player_id: source.id, game_id: games(:first).id, team_id: teams(:first).id, on: 0, off: 90, yellow: false, red: false)
    source_team_player = TeamPlayer.create!(player_id: source.id, team_id: teams(:first).id, championship_id: championships(:first).id)

    target.merge_player(source)

    assert_not Player.exists?(source.id)
    assert_equal target.id, goal.reload.player_id
    assert_equal target.id, source_player_game.reload.player_id
    assert_equal target.id, source_team_player.reload.player_id

    target.reload
    assert_equal 'Ana Maria', target.full_name
    assert_equal 'cm', target.position
    assert_equal Date.new(1990, 1, 1), target.birth
    assert_equal 'Brazil', target.country
  end

  def test_merge_player_uses_source_when_target_fields_are_blank
    target = Player.create!(name: 'Target 2', full_name: nil, position: nil, birth: nil, country: nil)
    source = Player.create!(name: 'Source 2', full_name: 'Long Source Name', position: 'dr', birth: Date.new(1995, 5, 5), country: 'Uruguay')

    target.merge_player(source)

    assert_not Player.exists?(source.id)

    target.reload
    assert_equal 'Long Source Name', target.full_name
    assert_equal 'dr', target.position
    assert_equal Date.new(1995, 5, 5), target.birth
    assert_equal 'Uruguay', target.country
  end
end
