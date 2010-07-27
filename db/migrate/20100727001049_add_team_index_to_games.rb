class AddTeamIndexToGames < ActiveRecord::Migration
  def self.up
    add_index(:games, :home_id)
    add_index(:games, :away_id)
  end

  def self.down
    remove_index(:games, :home_id)
    remove_index(:games, :away_id)
  end
end
