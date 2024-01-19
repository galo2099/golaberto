class MoveGeocodeDataToGeocodeTeamTable < ActiveRecord::Migration[6.1]
  def change
    Team.find_each do |team|
      TeamGeocode.create(data: team.geocode, team: team)
    end
  end
end
