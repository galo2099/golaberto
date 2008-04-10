class FixPlayerGame < ActiveRecord::Migration
  def self.up
    change_column :player_games, :on, :integer, :limit => 20, :default => 0, :null => false
    change_column :player_games, :off, :integer, :limit => 20, :default => 0, :null => false
    change_column :player_games, :yellow, :boolean, :default => false, :null => false
    change_column :player_games, :red, :boolean, :default => false, :null => false
  end

  def self.down
    change_column :player_games, :on, :integer, :limit => 2
    change_column :player_games, :off, :integer, :limit => 2
    change_column :player_games, :yellow, :integer, :limit => 2
    change_column :player_games, :red, :integer, :limit => 2
  end
end
