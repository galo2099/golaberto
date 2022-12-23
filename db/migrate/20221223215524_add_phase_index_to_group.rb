class AddPhaseIndexToGroup < ActiveRecord::Migration[6.1]
  def change
    add_index :groups, :phase_id
  end
end
