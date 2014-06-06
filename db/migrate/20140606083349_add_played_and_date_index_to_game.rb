class AddPlayedAndDateIndexToGame < ActiveRecord::Migration
  def change
    add_index(:games, :played)
    add_index(:games, :date)
  end
end
