class AddZonesToGroups < ActiveRecord::Migration
  def change
		reversible do |dir|
      change_table :groups do |t|
        dir.up do
          t.text :zones
          Group.all.each do |g|
            g.zones = []
            if g.promoted > 0 then
              g.zones.push({"name" => "Promoted", "color" => "#90EE90", "position" => (1..g.promoted).to_a })
            end
            if g.relegated > 0 then
              g.zones.push({"name" => "Relegated", "color" => "#FFA0A0", "position" => ((g.team_groups.size - g.relegated + 1)..g.team_groups.size).to_a })
            end
            g.save!
          end
        end
        dir.down do
          t.remove :zones
        end
      end
    end
  end
end
