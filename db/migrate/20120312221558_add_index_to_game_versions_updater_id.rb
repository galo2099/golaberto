class AddIndexToGameVersionsUpdaterId < ActiveRecord::Migration
  def change
    add_index :game_versions, :updater_id
  end
end
