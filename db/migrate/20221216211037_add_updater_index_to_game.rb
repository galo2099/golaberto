class AddUpdaterIndexToGame < ActiveRecord::Migration[6.1]
  def change
    add_index :games, [:updated_at, :updater_id]
  end
end
