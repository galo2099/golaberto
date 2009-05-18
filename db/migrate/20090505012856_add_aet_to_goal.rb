class AddAetToGoal < ActiveRecord::Migration
  def self.up
    add_column :goals, :aet, :boolean, :default => false, :null => false
  end

  def self.down
    remove_column :goals, :aet
  end
end
