class AddRatingsToTeam < ActiveRecord::Migration
  def change
    add_column :teams, :rating, :float
    add_column :teams, :off_rating, :float
    add_column :teams, :def_rating, :float
  end
end
