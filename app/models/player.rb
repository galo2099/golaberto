class Player < ApplicationRecord
  include Country

  has_many :comments, ->{order(created_at: :asc) }, :as => :commentable, :dependent => :destroy
  has_many :goals, :dependent => :delete_all
  has_many :team_players, :dependent => :delete_all
  has_many :player_games, :dependent => :delete_all
  has_many :games, through: :player_games

  validates_length_of :name, :within => 1..40

  Positions = %w(g dr dc dl dm cm am fw)
  validates_inclusion_of :position, :in => Positions, :allow_nil => true

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: position , SQL Definition:tinytext
  # Field: birth , SQL Definition:date
  # Field: country , SQL Definition:varchar(255)
  # Field: full_name , SQL Definition:varchar(255)

  def self.compare_position(a, b)
    if a.nil? and b.nil?
      0
    elsif a.nil?
      1
    elsif b.nil?
      -1
    else
      Positions.index(a) <=> Positions.index(b)
    end
  end

  def to_param
    "#{id}-#{name.parameterize}"
  end

  def small_country_logo
    Player.small_country_flag(country)
  end

  def merge_player(source_player)
    transaction do
      Goal.where(player_id: source_player.id).update_all(player_id: id)
      PlayerGame.where(player_id: source_player.id).update_all(player_id: id)
      TeamPlayer.where(player_id: source_player.id).update_all(player_id: id)

      self.full_name = [full_name, source_player.full_name].compact.max_by(&:length)
      self.position = source_player.position if position.blank?
      self.height = source_player.height if height.blank?
      self.birth = source_player.birth if birth.blank?
      self.country = source_player.country if country.blank?
      self.sofascore_id = source_player.sofascore_id if sofascore_id.blank?
      save!

      source_player.destroy!
    end
  end
end
