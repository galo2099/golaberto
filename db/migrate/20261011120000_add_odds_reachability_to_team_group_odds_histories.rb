class AddOddsReachabilityToTeamGroupOddsHistories < ActiveRecord::Migration[8.1]
  def change
    add_column :team_group_odds_histories, :odds_reachability, :text
  end
end
