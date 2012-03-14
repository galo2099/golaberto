class AddAttachmentLogoToTeam < ActiveRecord::Migration
  def self.up
    rename_column :teams, :logo, :legacy_logo
    add_column :teams, :logo_file_name, :string
    add_column :teams, :logo_content_type, :string
    add_column :teams, :logo_file_size, :integer
    add_column :teams, :logo_updated_at, :datetime
    Team.all.each do |team|
      unless team.legacy_logo.nil?
        team.logo = File.open("#{Rails.root}/public/images/logos/#{team.legacy_logo.gsub(/(.*)\.svg/, '\1_100.png')}")
        team.save!
      end
    end
  end

  def self.down
    remove_column :teams, :logo_file_name
    remove_column :teams, :logo_content_type
    remove_column :teams, :logo_file_size
    remove_column :teams, :logo_updated_at
  end
end
