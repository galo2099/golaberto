class AddChampionshipCategoryIdToGames < ActiveRecord::Migration[8.1]
  def up
    add_column :games, :championship_category_id, :integer, null: false, default: 0
    add_index :games, [:championship_category_id, :played, :date, :phase_id], name: "index_games_on_category_played_date_phase"

    execute <<~SQL
      UPDATE games
      INNER JOIN phases ON phases.id = games.phase_id
      INNER JOIN championships ON championships.id = phases.championship_id
      SET games.championship_category_id = championships.category_id
    SQL
  end

  def down
    remove_index :games, name: "index_games_on_category_played_date_phase"
    remove_column :games, :championship_category_id
  end
end
