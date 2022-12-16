class AddIndexToGoals < ActiveRecord::Migration[6.1]
  def change
    add_index :goals, :game_id
  end
end
