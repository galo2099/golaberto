class AddFullNameToTeam < ActiveRecord::Migration
  def self.up
    add_column :teams, :full_name, :string
  end

  def self.down
    remove_column :teams, :full_name
  end
end
