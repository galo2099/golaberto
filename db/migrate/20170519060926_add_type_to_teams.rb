class AddTypeToTeams < ActiveRecord::Migration
  def change
    add_column :teams, :team_type, :integer, null: false, default: 0
  end
end
