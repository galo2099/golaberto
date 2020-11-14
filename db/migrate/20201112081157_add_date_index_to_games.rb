class AddDateIndexToGames < ActiveRecord::Migration
  def change
    add_index :games, :date
  end
end
