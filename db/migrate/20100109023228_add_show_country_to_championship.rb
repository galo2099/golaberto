class AddShowCountryToChampionship < ActiveRecord::Migration
  def self.up
    add_column :championships, :show_country, :boolean, :null => false, :default => false
  end

  def self.down
    remove_column :championships, :show_country
  end
end
