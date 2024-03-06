class AddHeightToPlayers < ActiveRecord::Migration[7.1]
  def change
    add_column :players, :height, :integer
  end
end
