class AddInformationToUser < ActiveRecord::Migration
  def self.up
    add_column :users, :name, :string, :limit => 30
    add_column :users, :location, :string, :limit => 100
    add_column :users, :birthday, :date
    add_column :users, :about_me, :text, :limit => 2000
    add_column :users, :last_login, :datetime
  end

  def self.down
    remove_column :users, :name
    remove_column :users, :location
    remove_column :users, :birthday
    remove_column :users, :about_me
    remove_column :users, :last_login
  end
end
