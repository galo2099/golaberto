require 'test_helper'

class GroupTest < ActiveSupport::TestCase

  test 'can store and retrieve group with overlapping zone definitions' do
    # Setup: Create category, championship, and phase
    # Ensure a category exists or create one
    category = Category.first
    if category.nil?
      category = Category.create!(name: 'Default Category')
    end

    championship = Championship.create!(
      name: 'Test Championship',
      region: :national, # Using enum value
      category_id: category.id,
      begin: Date.today,
      end: Date.today + 30.days,
      point_win: 3,
      point_draw: 1,
      point_loss: 0
    )
    assert championship.persisted?, "Championship failed to save: #{championship.errors.full_messages.join(", ")}"

    phase = Phase.create!(
      name: 'Test Phase',
      championship: championship,
      order_by: 1,
      sort: 'pt,gd' # Common sort criteria
    )
    assert phase.persisted?, "Phase failed to save: #{phase.errors.full_messages.join(", ")}"

    group = Group.new(
      name: 'Test Group A',
      phase: phase
    )

    overlapping_zones_data = [
      { 'name' => 'Promotion', 'color' => '#00FF00', 'position' => [1, 2, 3, 4] },
      { 'name' => 'Playoff', 'color' => '#0000FF', 'position' => [4, 5, 6] }, # Position 4 overlaps
      { 'name' => 'Relegation', 'color' => '#FF0000', 'position' => [10, 11, 12] }
    ]

    group.zones = overlapping_zones_data
    assert group.save, "Group failed to save: #{group.errors.full_messages.join(", ")}"

    retrieved_group = Group.find(group.id)
    assert_equal overlapping_zones_data, retrieved_group.zones, 'Retrieved zones data does not match saved data'
  end

  test 'zones attribute is an Array' do
    category = Category.first || Category.create!(name: 'Default Category')
    championship = Championship.create!(
      name: 'Test Championship B', region: :national, category_id: category.id,
      begin: Date.today, end: Date.today + 30.days,
      point_win: 3, point_draw: 1, point_loss: 0
    )
    phase = Phase.create!(
      name: 'Test Phase B', championship: championship, order_by: 1, sort: 'pt'
    )
    group = Group.new(name: 'Test Group B', phase: phase)
    group.zones = [{ 'name' => 'Zone 1', 'color' => '#ABCDEF', 'position' => [1,2] }]
    group.save!
    retrieved_group = Group.find(group.id)
    assert_kind_of Array, retrieved_group.zones, "Zones attribute should be an Array"
  end

end
