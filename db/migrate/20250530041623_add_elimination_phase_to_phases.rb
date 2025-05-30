class AddEliminationPhaseToPhases < ActiveRecord::Migration[7.2]
  def change
    add_column :phases, :elimination_phase, :boolean, default: false
  end
end
