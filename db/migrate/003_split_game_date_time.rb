class SplitGameDateTime < ActiveRecord::Migration
  def self.up
    add_column :games, :new_date, :date
    add_column :games, :time, :time
    Game.reset_column_information
    say_with_time "Updating games" do
      games = Game.find(:all)
      games.each do |g|
        my_date = g.date
        my_time = unless g.date.nil? or (g.date.hour == 0 and g.date.min == 0)
                    g.date
                  end
        g.update_attributes(:new_date => my_date,
                            :time => my_time)
        say "#{g.id} updated!", true
      end
    end

    remove_column :games, :date
    rename_column :games, :new_date, :date
    change_column :games, :date, :date, :null => false
  end

  def self.down
    add_column :games, :new_date, :datetime
    Game.reset_column_information
    say_with_time "Updating games" do
      games = Game.find(:all)
      games.each do |g|
        my_date = g.date.nil? ? nil : g.date.to_time
        unless g.time.nil?
          my_date = my_date.change(:hour => g.time.hour, :min => g.time.min,
                                   :sec => g.time.sec, :usec => g.time.usec)
        end
        g.update_attribute(:new_date, my_date)
        say "#{g.id} updated!", true
      end
    end

    remove_column :games, :date
    remove_column :games, :time
    rename_column :games, :new_date, :date
    change_column :games, :date, :datetime, :null => false
  end
end
