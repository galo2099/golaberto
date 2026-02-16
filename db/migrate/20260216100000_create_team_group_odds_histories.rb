class CreateTeamGroupOddsHistories < ActiveRecord::Migration[7.2]
  def change
    create_table :team_group_odds_histories do |t|
      t.references :team_group, null: false, foreign_key: true, index: false
      t.date :recorded_on, null: false
      t.text :odds, null: false
      t.datetime :captured_at, null: false

      t.timestamps
    end

    add_index :team_group_odds_histories,
              [:team_group_id, :recorded_on],
              unique: true,
              name: "index_tg_odds_histories_on_team_group_and_day"
  end
end
