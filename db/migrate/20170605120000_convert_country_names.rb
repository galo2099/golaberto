class AddRegionToChampionship < ActiveRecord::Migration
  def change
    Team.where(country: "Brunei Darussalam").each{|t|t.country = "Brunei"; t.save}
    Team.where(country: "Samoa (Independent)").each{|t|t.country = "Samoa"; t.save}
    Team.where(country: "Congo, the Democratic Republic of the").each{|t|t.country = "DR Congo"; t.save}
  end
end

