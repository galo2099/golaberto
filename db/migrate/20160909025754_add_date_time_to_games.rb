class AddDateTimeToGames < ActiveRecord::Migration
  def up
    [:games, :game_versions].each do |g|
      add_column g, :new_date, :datetime
      add_column g, :has_time, :boolean, default: false

      Game.connection.execute("update #{g} set has_time = (time is not null)")
      Game.connection.execute("update #{g} set new_date = date where time is null")
      Game.connection.execute("update #{g} set new_date = ADDTIME(date, time) where time is not null")

      remove_column g, :date
      remove_column g, :time
      rename_column g, :new_date, :date
      change_column g, :date, :datetime, :null => false
    end
  end

  def down
    [:games, :game_versions].each do |g|
      add_column g, :new_date, :date
      add_column g, :time, :time

      Game.connection.execute("update #{g} set time = date where has_time = true")
      Game.connection.execute("update #{g} set new_date = date")

      remove_column g, :date
      remove_column g, :has_time
      rename_column g, :new_date, :date
      change_column g, :date, :date, :null => false
    end
  end
end
