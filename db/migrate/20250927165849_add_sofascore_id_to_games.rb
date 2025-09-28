class AddSofascoreIdToGames < ActiveRecord::Migration[7.2]
  def change
    add_column :games, :sofascore_id, :string
    add_column :game_versions, :sofascore_id, :string
  end
end
