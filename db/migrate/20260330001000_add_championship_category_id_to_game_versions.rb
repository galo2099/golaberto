class AddChampionshipCategoryIdToGameVersions < ActiveRecord::Migration[8.0]
  def up
    add_column :game_versions, :championship_category_id, :integer, null: false, default: 0
  end

  def down
    remove_column :game_versions, :championship_category_id
  end
end
