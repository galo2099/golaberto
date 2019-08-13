class AddImportanceToGames < ActiveRecord::Migration
  def change
    add_column :games, :home_importance, :float
    add_column :games, :away_importance, :float
    add_column :game_versions, :home_importance, :float
    add_column :game_versions, :away_importance, :float
  end
end
