class AddSofascoreIdToPlayers < ActiveRecord::Migration[7.2]
  def change
    add_column :players, :sofascore_id, :string
  end
end
