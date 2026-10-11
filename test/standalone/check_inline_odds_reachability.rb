require 'minitest/autorun'
require 'active_record'
require 'active_support/concern'

# Isolated database: exercise migration up/down without altering Rails databases.
database_url = ENV['ODDS_TEST_DATABASE_URL']
if database_url
  ActiveRecord::Base.establish_connection(database_url)
else
  ActiveRecord::Base.establish_connection(adapter: 'sqlite3', database: ':memory:')
end
ActiveRecord::Schema.define do
  create_table :team_groups, force: true do |t|
    t.integer :team_id
    t.integer :group_id
    t.text :odds
    t.text :odds_reachability
    t.integer :add_sub
    t.integer :bias
    t.datetime :created_at
    t.datetime :updated_at
  end
end

require_relative '../../app/models/concerns/compact_odds'
require_relative '../../app/models/application_record'
require_relative '../../app/models/team_group'
require_relative '../../db/migrate/20261011121000_inline_team_group_odds_reachability'

class InlineOddsReachabilityMigrationTest < Minitest::Test
  def setup
    @now = Time.utc(2025, 4, 5, 6, 7, 8)
    @proven = legacy_team_group(
      1, [0.0, 0.0, 0.0, 75.0], %w[impossible reachable undecided reachable]
    )
    @partial = legacy_team_group(
      2, [[0.0, "reachable"], 0.0, [0.0, "undecided"]], %w[undecided impossible reachable]
    )
    @empty = legacy_team_group(3, nil, nil)
    @short_statuses = legacy_team_group(4, [75.0, 0.0, 0.0], ["reachable"])
    @missing_statuses = legacy_team_group(5, [75.0, 0.0], nil)
  end

  def test_up_down_up_preserves_odds_proofs_nil_values_and_timestamps
    assert_raises(RuntimeError) { migrate_with_failure(:up) }
    assert_nil legacy_row(@proven.id).odds_reachability
    assert_equal [[0.0, "reachable"], [0.0, "impossible"], 0.0], legacy_row(@partial.id).odds
    assert_nil legacy_row(@partial.id).odds_reachability

    migrate(:up)
    TeamGroup.reset_column_information

    refute_includes TeamGroup.column_names, "odds_reachability"
    @proven.reload
    assert_equal [[0.0, "impossible"], [0.0, "reachable"], 0.0, 75.0], @proven.odds
    assert_equal [0.0, 0.0, 0.0, 75.0], @proven.numeric_odds
    assert_equal %w[impossible reachable undecided reachable], @proven.odds_reachability
    assert_equal @now, @proven.created_at
    assert_equal @now, @proven.updated_at

    @partial.reload
    assert_equal [[0.0, "reachable"], [0.0, "impossible"], 0.0], @partial.odds
    assert_equal %w[reachable impossible undecided], @partial.odds_reachability

    @short_statuses.reload
    assert_equal [75.0, 0.0, 0.0], @short_statuses.odds
    assert_equal %w[reachable undecided undecided], @short_statuses.odds_reachability
    @missing_statuses.reload
    assert_equal [75.0, 0.0], @missing_statuses.odds
    assert_equal %w[reachable undecided], @missing_statuses.odds_reachability

    @empty.reload
    assert_nil @empty.odds
    assert_nil @empty.odds_reachability

    assert_raises(RuntimeError) { migrate_with_failure(:down) }
    assert_equal %w[impossible reachable undecided reachable], legacy_row(@proven.id).odds_reachability
    assert_equal %w[reachable impossible undecided], legacy_row(@partial.id).odds_reachability
    migrate(:down)
    TeamGroup.reset_column_information
    assert_includes TeamGroup.column_names, "odds_reachability"
    legacy = InlineTeamGroupOddsReachability::TeamGroup.find(@proven.id)
    assert_equal [0.0, 0.0, 0.0, 75.0], legacy.odds
    assert_equal %w[impossible reachable undecided reachable], legacy.odds_reachability
    assert_equal @now, legacy.updated_at
    empty = InlineTeamGroupOddsReachability::TeamGroup.find(@empty.id)
    assert_nil empty.odds
    assert_nil empty.odds_reachability

    migrate(:up)
    TeamGroup.reset_column_information
    assert_equal [[0.0, "impossible"], [0.0, "reachable"], 0.0, 75.0], @proven.reload.odds
    assert_equal %w[impossible reachable undecided reachable], @proven.odds_reachability
    refute_includes TeamGroup.column_names, "odds_reachability"
  end

  private

  def legacy_team_group(team_id, odds, statuses)
    team_group = InlineTeamGroupOddsReachability::TeamGroup.new(
      team_id: team_id,
      group_id: 1,
      odds: odds,
      odds_reachability: statuses,
      add_sub: 0,
      bias: 0,
      created_at: @now,
      updated_at: @now
    )
    team_group.save!(validate: false)
    TeamGroup.find(team_group.id)
  end

  def migrate(direction)
    InlineTeamGroupOddsReachability.new.migrate(direction)
    InlineTeamGroupOddsReachability::TeamGroup.reset_column_information
  end

  def migrate_with_failure(direction)
    model = InlineTeamGroupOddsReachability::TeamGroup
    original_find_each = model.method(:find_each)
    model.singleton_class.define_method(:find_each) do |batch_size:, &block|
      updates = 0
      rows = model.order(:id).to_a
      rows.each do |row|
        original_update_columns = row.method(:update_columns)
        row.define_singleton_method(:update_columns) do |**attributes|
          updates += 1
          raise "simulated write failure" if updates == 3
          original_update_columns.call(**attributes)
        end
      end
      rows.each(&block)
    end
    begin
      InlineTeamGroupOddsReachability.new.migrate(direction)
    ensure
      model.singleton_class.define_method(:find_each, original_find_each)
    end
  end

  def legacy_row(id)
    InlineTeamGroupOddsReachability::TeamGroup.find(id)
  end
end
