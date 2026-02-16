class CreateHistoricalOdds < ActiveRecord::Migration[7.2]
  def up
    create_table :historical_odds do |t|
      t.references :team_group, null: false, foreign_key: true
      t.date :measure_date, null: false
      t.text :odds
      t.timestamps
    end
    add_index :historical_odds, [:team_group_id, :measure_date], unique: true

    # Backfill current odds
    TeamGroup.where.not(odds: nil).each do |tg|
      HistoricalOdds.create(team_group_id: tg.id, measure_date: Date.today, odds: tg.odds)
    end
  end

  def down
    drop_table :historical_odds
  end
end
