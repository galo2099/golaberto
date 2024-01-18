class AddGeocodeToTeams < ActiveRecord::Migration[6.1]
  def change
    add_column :teams, :geocode, :json
  end
end
