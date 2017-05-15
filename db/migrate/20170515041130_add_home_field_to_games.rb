class AddHomeFieldToGames < ActiveRecord::Migration
  def change
    [ :games, :game_versions ].each do |table|
      change_table table do |t|
        t.integer :home_field, null: false, default: 0
      end
    end
  end
end
