class AddAttachmentAvatarToUser < ActiveRecord::Migration
  def self.up
    add_column :users, :avatar_file_name, :string
    add_column :users, :avatar_content_type, :string
    add_column :users, :avatar_file_size, :integer
    add_column :users, :avatar_updated_at, :datetime
    User.all.each do |user|
      user.avatar = File.open("#{Rails.root}/public/images/users/#{user.id}_100.png")
      user.save!
    end
  end

  def self.down
    remove_column :users, :avatar_file_name
    remove_column :users, :avatar_content_type
    remove_column :users, :avatar_file_size
    remove_column :users, :avatar_updated_at
  end
end
