class AddPercentageToTeamGroups < ActiveRecord::Migration
  def self.up
    add_column :team_groups, :first_odds, :float
    add_column :team_groups, :promoted_odds, :float
    add_column :team_groups, :relegated_odds, :float
  end

  def self.down
    remove_column :team_groups, :first_odds
    remove_column :team_groups, :promoted_odds
    remove_column :team_groups, :relegated_odds
  end
end
