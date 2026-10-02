class AddOddsReachabilityToTeamGroups < ActiveRecord::Migration[8.1]
  def change
    add_column :team_groups, :odds_reachability, :text
  end
end
