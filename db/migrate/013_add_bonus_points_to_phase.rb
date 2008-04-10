class AddBonusPointsToPhase < ActiveRecord::Migration
  def self.up
    add_column :phases, :bonus_points, :integer, :limit => 4, :default => 0, :null => false
    add_column :phases, :bonus_points_threshold, :integer, :limit => 4, :default => 0, :null => false
  end

  def self.down
    remove_column :phases, :bonus_points
    remove_column :phases, :bonus_points_threshold
  end
end
