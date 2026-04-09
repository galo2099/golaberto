class AddChampionshipCategoryIdToGames < ActiveRecord::Migration[8.0]
  def up
    add_column :games, :championship_category_id, :integer, null: false, default: 0

    execute <<~SQL
      UPDATE games g
      INNER JOIN phases p ON p.id = g.phase_id
      INNER JOIN championships c ON c.id = p.championship_id
      SET g.championship_category_id = c.category_id
    SQL

    add_index :games, [:championship_category_id, :played, :date], name: "index_games_on_category_played_and_date"
  end

  def down
    remove_index :games, name: "index_games_on_category_played_and_date"
    remove_column :games, :championship_category_id
  end
end
