class AddRegionToChampionship < ActiveRecord::Migration
  def change
    add_column :championships, :region, :integer, null: false, default: 0
    add_column :championships, :region_name, :text
    add_column :championships, :championship_type, :integer, null: false, default: 0

    Championship.all.each{|c|countries = c.teams.pluck(:country).sort.uniq; if countries.size == 1 then c.region_name = countries.first; c.national! end }
    Championship.where("name LIKE 'England%'").each{|c|c.region_name = "England"; c.national!}
    Championship.where("name = 'FA Cup'").each{|c|c.region_name = "England"; c.national!}
    Championship.where("name = 'EFL Cup'").each{|c|c.region_name = "England"; c.national!}
    Championship.where("name LIKE 'France%'").each{|c|c.region_name = "France"; c.national!}
    Championship.where("name LIKE 'Ireland%'").each{|c|c.region_name = "Ireland"; c.national!}
    Championship.where("name LIKE 'MLS%'").each{|c|c.region_name = "United States"; c.national!}
    Championship.where("name LIKE 'Swiss%'").each{|c|c.region_name = "Switzerland"; c.national!}

    Championship.world.each{|c|p c.full_name; continents = c.teams.map{|t| p t.country; ApplicationHelper::Continent.country_to_continent[t.country].name}.sort.uniq; if continents.size == 1 then c.region_name = continents.first; c.continental! end }
    Championship.world.where("name LIKE 'Copa América%'").each{|c|c.region_name = "South America"; c.continental!}
    Championship.world.where("name LIKE 'Copa Libertadores%'").each{|c|c.region_name = "South America"; c.continental!}
    Championship.world.where("name LIKE 'Copa Sudamericana%'").each{|c|c.region_name = "South America"; c.continental!}

    Championship.world.each{|c|c.region_name = "World"; c.save}
  end
end
