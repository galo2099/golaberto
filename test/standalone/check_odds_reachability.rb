require 'minitest/autorun'
require 'active_record'

# Isolated database: exercise serialization and the actual migration without
# altering development data or loading unrelated Rails fixtures.
ActiveRecord::Base.establish_connection(adapter: 'sqlite3', database: ':memory:')
ActiveRecord::Schema.define do
  create_table :groups
  create_table :team_groups do |t|
    t.integer :team_id
    t.integer :group_id
    t.text :odds
    t.integer :add_sub
    t.integer :bias
  end
end
require_relative '../../db/migrate/20261001120000_add_odds_reachability_to_team_groups'
AddOddsReachabilityToTeamGroups.new.migrate(:up)
require_relative '../../app/models/application_record'
class Group < ApplicationRecord; end
require_relative '../../app/models/team_group'

class OddsReachabilityTest < Minitest::Test
  def test_response_assignment_persists_aligned_statuses_and_clears_old_proofs
    team = TeamGroup.new(team_id: 5)
    response = { 'team_odds' => { '5' => { 'Pos' => [0, 0, 0, 100] } },
                 'rare_position_estimates' => { '5' => {
                   '0' => { 'reachability' => 'impossible' },
                   '1' => { 'reachability' => 'reachable' },
                   '2' => { 'reachability' => 'undecided' },
                   '3' => { 'reachability' => 'reachable' }
                 } } }
    team.assign_calculated_odds!(response)
    team.save!(validate: false)
    team.reload
    assert_equal %w[impossible reachable undecided reachable], team.odds_reachability
    assert_equal 'impossible', team.odds_reachability_for([1])
    assert_equal 'reachable', team.odds_reachability_for([1, 2])
    assert_equal 'undecided', team.odds_reachability_for([1, 3])
    assert_equal 'reachable', team.odds_reachability_for([3, 4])
    team.assign_calculated_odds!(response.reject{|key, _| key == 'rare_position_estimates'})
    assert_nil team.odds_reachability
    assert_equal 'undecided', team.odds_reachability_for([1])
  end

  def test_zero_is_not_evidence_for_certainty
    odds = [99.9999999999, 0, 0]
    assert_equal odds[0], TeamGroup.calculate_odds_for(odds, [1])
    assert_equal odds[0], TeamGroup.calculate_odds_for(odds, [1], %w[reachable reachable impossible])
    assert_equal 100.0, TeamGroup.calculate_odds_for(odds, [1], %w[reachable impossible impossible])
    assert_equal 100.0, TeamGroup.calculate_odds_for(odds, [1, 2, 3])
  end
end
