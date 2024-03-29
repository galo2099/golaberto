require 'poisson'
class Team < ApplicationRecord
  include Country
  enum team_type: [ :club, :national ]

  AVG_BASE = 1.3350257653834494

  has_one :team_geocode, dependent: :destroy
  has_many :historical_ratings

  has_attached_file :logo,
      styles: lambda { |attachment|
        options = { format: "png", filter_background: attachment.instance.filter_image_background? }
        { medium: options.merge(geometry: "100x100"),
          thumb: options.merge(geometry: "15x15") }
      },
      processors: [ :logo ],
      default_url: "#{Rails.configuration.golaberto_image_url_prefix}/:style.png",
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

  after_save :retrieve_geocode

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
    Team.small_country_flag(country)
  end

  def large_country_logo
    Team.large_country_flag(country)
  end

  def next_n_games(n, date)
    games.joins(phase: :championship).where(championships: { category_id: Category::DEFAULT_CATEGORY }).where("date >= ?", date).order(date: :asc).limit(n)
  end

  def last_n_games(n, date)
    games.joins(phase: :championship).where(championships: { category_id: Category::DEFAULT_CATEGORY }).where("date < ?", date).order(date: :desc).limit(n)
  end

  def self.get_historical_ratings_2_weeks(team_id)
    HistoricalRating.connection.select_all("select LEAST(FROM_UNIXTIME((unix_timestamp(measure_date) div (#{2.weeks.to_i}) + 1) * (#{2.weeks.to_i})), NOW()) as d, AVG(rating) as r from historical_ratings where team_id=#{team_id} group by d order by measure_date").to_a.map{|x|x.map{|k,v| v }}.map{|x| x[0] = x[0].to_time.to_i; x[1] = x[1].to_f; x[1] = nil if x[1] == 0; x}
  end

  def retrieve_geocode
    url = "https://nominatim.openstreetmap.org/search.php?q=#{CGI.escape(city.to_s + ", " + country)}&format=jsonv2&namedetails=1&layer=address"
    uri = URI(url)
    response = Net::HTTP.get(uri)
    self.build_team_geocode unless self.team_geocode
    self.team_geocode.update(data: JSON.parse(response))
    return true
  end
end
