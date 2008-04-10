class AddGameTimestampAndUserstamp < ActiveRecord::Migration
  def self.up
    add_column :games, :updater_id, :integer, :limit => 20, :default => 0, :null => false
    add_column :games, :updated_at, :datetime, :null => false
    add_column :game_versions, :updater_id, :integer, :limit => 20, :default => 0, :null => false
    add_column :game_versions, :updated_at, :datetime, :null => false
  end

  def self.down
    remove_column :games, :updater_id
    remove_column :games, :updated_at
    remove_column :game_versions, :updater_id
    remove_column :game_versions, :updated_at
  end
end
