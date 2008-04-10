class AddGameVersion < ActiveRecord::Migration
  def self.up
    Game.create_versioned_table
    create_table :game_goals_versions, :id => false, :force => true do |t|
      t.column :game_version_id, :integer, :default => 0, :null => false
      t.column :goal_id, :integer, :default => 0, :null => false
    end
    Game.reset_column_information
    Game.class_eval do
      def changed?
        true
      end
    end
    games = Game.find(:all)
    say_with_time "Updating games" do
      games.each do |g|
        g.save
        say "#{g.id} updated!", true
      end
    end
  end

  def self.down
    Game.drop_versioned_table
    remove_column :games, :version
    drop_table :game_goals_versions
  end
end
