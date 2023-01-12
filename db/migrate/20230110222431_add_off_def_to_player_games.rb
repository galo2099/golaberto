class AddOffDefToPlayerGames < ActiveRecord::Migration[6.1]
  def change
    add_column :player_games, :off_rating, :float
    add_column :player_games, :def_rating, :float
  end
end
