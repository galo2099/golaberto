class AddIndexToPlayerGames < ActiveRecord::Migration
  def change
    add_index :player_games, [:player_id, :game_id, :team_id], unique: true
  end
end
