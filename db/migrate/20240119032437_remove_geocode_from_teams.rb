class RemoveGeocodeFromTeams < ActiveRecord::Migration[6.1]
  def change
    remove_column :teams, :geocode, :json
  end
end
