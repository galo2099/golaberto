class AddPhaseIdIndexToGame < ActiveRecord::Migration
  def change
    add_index :games, :phase_id
  end
end
