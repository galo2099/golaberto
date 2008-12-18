class CreateRoles < ActiveRecord::Migration
  def self.up
    create_table "roles" do |t|
      t.string :name
    end

    # generate the join table
    create_table "roles_users", :id => false do |t|
      t.integer "role_id", "user_id"
    end
    add_index "roles_users", "role_id"
    add_index "roles_users", "user_id"
    editor = Role.create({:name => "editor"})
    commenter = Role.create({:name => "commenter"})
    User.find(:all).each do |u|
      u.roles << editor
      u.roles << commenter
    end
  end

  def self.down
    drop_table "roles"
    drop_table "roles_users"
  end
end
