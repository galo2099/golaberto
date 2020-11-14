class AddOffDefToHistoricalRatings < ActiveRecord::Migration
  def change
    add_column :historical_ratings, :off_rating, :float, null: false
    add_column :historical_ratings, :def_rating, :float, null: false
  end
end
