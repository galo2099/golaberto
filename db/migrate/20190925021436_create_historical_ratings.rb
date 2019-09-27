class CreateHistoricalRatings < ActiveRecord::Migration
  def change
    create_table :historical_ratings do |t|
      t.references :team, index: true, foreign_key: true, null: false
      t.float :rating, null: false
      t.date :measure_date, null: false
      t.index [:team_id, :measure_date], unique: true
    end
  end
end
