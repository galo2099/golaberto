class AllowNullGameIdInGoal < ActiveRecord::Migration
  def self.up
    change_column :goals, :game_id, :integer, :limit => 20
  end

  def self.down
    change_column :goals, :game_id, :integer, :limit => 20, :default => 0, :null => false
  end
end
