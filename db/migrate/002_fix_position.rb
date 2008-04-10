class FixPosition < ActiveRecord::Migration
  def self.up
    change_column "players", "position", :string, :limit => 3
  end

  def self.down
    change_column "players", "position", :text
  end
end
