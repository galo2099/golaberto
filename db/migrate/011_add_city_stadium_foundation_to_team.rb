class AddCityStadiumFoundationToTeam < ActiveRecord::Migration
  def self.up
    add_column :teams, :city, :string
    add_column :teams, :stadium_id, :integer, :limit => 20
    add_column :teams, :foundation, :date
  end

  def self.down
    remove_column :teams, :city
    remove_column :teams, :stadium_id
    remove_column :teams, :foundation
  end
end
