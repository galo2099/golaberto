class AddPlayerGamesGameFk < ActiveRecord::Migration[8.0]
  def up
    execute <<~SQL
      DELETE pg
      FROM player_games pg
      LEFT JOIN games g ON g.id = pg.game_id
      WHERE g.id IS NULL
    SQL

    change_column_default :player_games, :game_id, from: 0, to: nil

    add_foreign_key :player_games, :games, column: :game_id, on_delete: :cascade
  end

  def down
    remove_foreign_key :player_games, column: :game_id

    change_column_default :player_games, :game_id, from: nil, to: 0
  end
end
