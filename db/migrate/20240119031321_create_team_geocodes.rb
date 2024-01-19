class CreateTeamGeocodes < ActiveRecord::Migration[6.1]
  def change
    create_table :team_geocodes do |t|
      t.json :data
      t.references :team, null: false, foreign_key: true, type: :integer
    end
  end
end
