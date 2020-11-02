class AddSoccerwayIdToGames < ActiveRecord::Migration
  def change
    add_column :games, :soccerway_id, :string
    add_index :games, :soccerway_id, unique: true
    add_column :game_versions, :soccerway_id, :string
  end
end
