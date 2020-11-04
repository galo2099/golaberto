class AddIndexToPlayerGames < ActiveRecord::Migration
  def change
    add_index :player_games, [:game_id, :team_id, :player_id], unique: true
  end
end
