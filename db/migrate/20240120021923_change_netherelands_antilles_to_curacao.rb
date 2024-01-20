class ChangeNetherelandsAntillesToCuracao < ActiveRecord::Migration[6.1]
  def change
    Team.where(country: "Netherlands Antilles").each do |t|
      t.country = "Curacao"
      t.save
    end
    Player.where(country: "Netherlands Antilles").each do |p|
      p.country = "Curacao"
      p.save
    end
  end
end
