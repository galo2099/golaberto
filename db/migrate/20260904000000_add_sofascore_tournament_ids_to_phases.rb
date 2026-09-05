class AddSofascoreTournamentIdsToPhases < ActiveRecord::Migration[8.1]
  def change
    add_column :phases, :sofascore_tournament_ids, :string
  end
end
