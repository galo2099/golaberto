class AddGameIdIndexToGameVersions < ActiveRecord::Migration
  def change
    add_index(:game_versions, :game_id)
  end
end
