class AddAetGoalsToGameVersion < ActiveRecord::Migration
  def self.up
    add_column :game_versions, :home_aet, :integer
    add_column :game_versions, :away_aet, :integer
  end

  def self.down
    remove_column :game_versions, :away_aet
    remove_column :game_versions, :home_aet
  end
end
