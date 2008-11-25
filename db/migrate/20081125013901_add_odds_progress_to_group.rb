class AddOddsProgressToGroup < ActiveRecord::Migration
  def self.up
    add_column :groups, :odds_progress, :integer
  end

  def self.down
    remove_column :groups, :odds_progress
  end
end
