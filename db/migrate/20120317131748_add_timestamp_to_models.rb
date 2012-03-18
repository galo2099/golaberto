class AddTimestampToModels < ActiveRecord::Migration
  TABLES_TO_MODIFY = [ :championships, :goals, :phases, :groups, :players, :referees, :stadia, :team_groups, :team_players, :teams ]
  def up
    TABLES_TO_MODIFY.each do |table|
      change_table table do |t|
        t.timestamps
      end
      model = table.to_s.classify.constantize
      model.reset_column_information
      say_with_time "Touching #{table}..." do
        model.all.map do |o|
          o.touch
        end
      end
    end
  end

  def down
    TABLES_TO_MODIFY.each do |table|
      change_table table do |t|
        t.remove_timestamps
      end
    end
  end
end
