class AddFullNameAndCityToStadium < ActiveRecord::Migration
  def self.up
    add_column :stadia, :full_name, :string, :limit => 255, :null => true
    add_column :stadia, :city, :string, :limit => 255, :null => true
    add_column :stadia, :country, :string, :limit => 255, :null => true
  end

  def self.down
   remove_column :stadia, :full_name
   remove_column :stadia, :city
   remove_column :stadia, :country
  
  end
end
