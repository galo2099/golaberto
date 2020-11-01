class AddSoccerwayIdToPlayers < ActiveRecord::Migration
  def change
    add_column :players, :soccerway_id, :string
    add_index :players, :soccerway_id, unique: true
  end
end
