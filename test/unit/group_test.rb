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


  test 'odds backfill mode only writes odds history' do
    category = Category.create!(name: 'Backfill Category')
    championship = Championship.create!(
      name: 'Backfill Championship',
      region: :national,
      category_id: category.id,
      begin: Date.today,
      end: Date.today + 30.days,
      point_win: 3,
      point_draw: 1,
      point_loss: 0
    )
    phase = Phase.create!(
      name: 'Backfill Phase',
      championship: championship,
      order_by: 1,
      sort: 'pt,gd'
    )
    group = Group.create!(name: 'Backfill Group', phase: phase)
    home = Team.create!(name: 'Backfill FC', country: 'Brazil')
    away = Team.create!(name: 'Snapshot United', country: 'Brazil')
    tg_home = TeamGroup.create!(group: group, team: home, add_sub: 0, bias: 0)
    tg_away = TeamGroup.create!(group: group, team: away, add_sub: 0, bias: 0)

    response = Struct.new(:body).new({
      'team_odds' => {
        home.id.to_s => { 'Pos' => [0.65, 0.35] },
        away.id.to_s => { 'Pos' => [0.35, 0.65] }
      },
      'game_importance' => {}
    }.to_json)

    fake_http = Object.new
    fake_http.define_singleton_method(:request) { |_req| response }
    fake_client = Object.new
    fake_client.define_singleton_method(:start) { |&block| block.call(fake_http) }

    group_progress_before = group.odds_progress

    assert_difference 'TeamGroupOddsHistory.count', +2 do
      Net::HTTP.stub(:new, fake_client) do
        group.odds(
          games_json: [],
          snapshot_time: Time.zone.parse('2024-06-01 10:00:00'),
          persist_game_importance: false,
          persist_team_odds: false,
          persist_group_progress: false
        )
      end
    end

    assert_nil tg_home.reload.odds
    assert_nil tg_away.reload.odds
    if group_progress_before.nil?
      assert_nil group.reload.odds_progress
    else
      assert_equal group_progress_before, group.reload.odds_progress
    end
  end

end
