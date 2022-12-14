require 'poisson'
class Team < ApplicationRecord
  enum team_type: [ :club, :national ]

  AVG_BASE = 1.3350257653834494

  has_many :historical_ratings

  has_attached_file :logo,
      styles: lambda { |attachment|
        options = { format: "png", filter_background: attachment.instance.filter_image_background? }
        { medium: options.merge(geometry: "100x100"),
          thumb: options.merge(geometry: "15x15") }
      },
      processors: [ :logo ],
      storage: :s3,
      s3_region: 'us-east-1',
      s3_credentials: Rails.application.secrets.s3,
      s3_headers: { 'Cache-Control' => 'max-age=315576000', 'Expires' => 10.years.from_now.httpdate },
      default_url: 'https://s3.amazonaws.com/:bucket/:style.png',
      path: ":class/:attachment/:id/:style.:extension"
  validates_attachment :logo, content_type: { content_type: ["image/jpg", "image/jpeg", "image/png", "image/gif"] }

  # Virtual attribute to see if we should filter the image background
  attr_accessor :filter_image_background

  belongs_to :stadium, optional: true
  has_many :comments, ->{ order(created_at: :asc) }, :as => :commentable, :dependent => :destroy
  has_many :team_groups, :dependent => :delete_all
  has_many :groups, :through => :team_groups
  has_many :home_games, :foreign_key => "home_id", :class_name => "Game", :dependent => :destroy
  has_many :away_games, :foreign_key => "away_id", :class_name => "Game", :dependent => :destroy
  has_many :team_players, ->{ includes :player}, :dependent => :delete_all
  validates_length_of :name, :within => 1..40
  validates_length_of :country, :within => 1..40
  validates_uniqueness_of :name, :message => "already exists"

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: country , SQL Definition:varchar(255)
  # Field: logo , SQL Definition:varchar(255)

  def self.i18n_team_types
    hash = {}
    team_types.keys.each { |key| hash[I18n.t("activerecord.attributes.team.team_type.#{key}")] = key }
    hash
  end

  def filter_image_background?
    return filter_image_background == "1"
  end

  def to_param
    "#{id}-#{name.parameterize}"
  end

  def games
    Game.where("home_id = ? OR away_id = ?", self.id, self.id)
  end

  def small_country_logo
    if country.nil?
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3[:bucket]}/thumb.png"
    else
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3[:bucket]}/countries/flags/#{country.parameterize(separator: '_')}_15.png"
    end
  end

  def large_country_logo
    if country.nil?
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3[:bucket]}/medium.png"
    else
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3[:bucket]}/countries/flags/#{country.parameterize(separator: '_')}_100.png"
    end
  end

  def next_n_games(n, date)
    games.joins(phase: :championship).where(championships: { category_id: Category::DEFAULT_CATEGORY }).where("date >= ?", date).order(date: :asc).limit(n)
  end

  def last_n_games(n, date)
    games.joins(phase: :championship).where(championships: { category_id: Category::DEFAULT_CATEGORY }).where("date < ?", date).order(date: :desc).limit(n)
  end
end
