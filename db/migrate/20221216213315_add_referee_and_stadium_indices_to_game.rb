class AddRefereeAndStadiumIndicesToGame < ActiveRecord::Migration[6.1]
  def change
    add_index :games, :stadium_id
    add_index :games, :referee_id
  end
end
