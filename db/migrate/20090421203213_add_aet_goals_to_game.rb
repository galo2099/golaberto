class AddAetGoalsToGame < ActiveRecord::Migration
  def self.up
    add_column :games, :home_aet, :integer
    add_column :games, :away_aet, :integer
  end

  def self.down
    remove_column :games, :away_aet
    remove_column :games, :home_aet
  end
end
