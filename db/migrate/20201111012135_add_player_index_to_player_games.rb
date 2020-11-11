class AddPlayerIndexToPlayerGames < ActiveRecord::Migration
  def change
    add_index :player_games, :player_id
  end
end
