require File.dirname(__FILE__) + '/../test_helper'

class GameTest < Test::Unit::TestCase
  def test_sets_championship_category_id_from_phase_on_validation
    championship = championships(:first)
    championship.update!(:category => categories(:one))
    phase = championship.phases.create!(:name => "Fase única", :order_by => 1, :sort => "points")

    game = phase.games.create!(
      :home => teams(:first),
      :away => teams(:another),
      :date => Time.utc(2026, 1, 1),
      :played => false
    )

    assert_equal categories(:one).id, game.championship_category_id
  end

  def test_updates_existing_games_when_championship_category_changes
    championship = championships(:first)
    championship.update!(:category => categories(:one))
    phase = championship.phases.create!(:name => "Fase final", :order_by => 2, :sort => "points")
    game = phase.games.create!(
      :home => teams(:first),
      :away => teams(:another),
      :date => Time.utc(2026, 1, 2),
      :played => true
    )

    championship.update!(:category => categories(:two))

    assert_equal categories(:two).id, game.reload.championship_category_id
  end
end
