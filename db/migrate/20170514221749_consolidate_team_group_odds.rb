class ConsolidateTeamGroupOdds < ActiveRecord::Migration
  def change
    change_table :team_groups do |t|
      t.remove :first_odds
      t.remove :promoted_odds
      t.remove :relegated_odds
      t.text :odds
    end
  end
end
