class AddInformationToUser < ActiveRecord::Migration
  def self.up
    add_column :users, :name, :string, :limit => 30
    add_column :users, :website, :string, :limit => 100
    add_column :users, :location, :string, :limit => 100
    add_column :users, :birthday, :date
    add_column :users, :about_me, :text, :limit => 2000
  end

  def self.down
    remove_column :users, :name
    add_column :users, :website
    add_column :users, :location
    add_column :users, :birthday
    add_column :users, :about_me
  end
end
