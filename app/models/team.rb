require 'poisson'
require 'lttb'
class Team < ApplicationRecord
  include Country
  enum :team_type, [ :club, :national ]

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

  def self.get_historical_ratings(team_id, threshold = nil)
    data = HistoricalRating.where(team_id: team_id).order(:measure_date).pluck(:measure_date, :rating).map { |d, r| [d.to_time.to_i, r.to_f] }
    return data if threshold.nil?

    LTTB.downsample(data, threshold)
  end

  def self.update_ratings
    all_games = Game.joins(phase: :championship)
      .select(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field)
      .where(championships: { category_id: 1 }, played: true)
      .where("date >= ?", DateTime.now - 4.years)
      .where("date <= ?", DateTime.now)
      .reorder(:date)

    json_map = {
      games: all_games.pluck(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field)
        .map { |home_id, away_id, phase_id, home_score, home_aet, away_score, away_aet, date, home_field|
          {
            home_id: home_id,
            away_id: away_id,
            phase_id: phase_id,
            home_score: (home_score + home_aet.to_i).to_f / (home_aet.nil? ? 1.0 : 4.0 / 3.0),
            away_score: (away_score + away_aet.to_i).to_f / (home_aet.nil? ? 1.0 : 4.0 / 3.0),
            timestamp: date.to_i,
            length: home_aet.nil? ? 1.0 : 4.0 / 3.0,
            advantage: if home_field == Game.home_fields["left"] then Game::HOME_ADV elsif home_field == Game.home_fields["neutral"] then 0.0 else -Game::HOME_ADV end
          }
        },
      ratings: Team.all.pluck(:id, :off_rating, :def_rating).map { |id, off_rating, def_rating| { id: id, offense: off_rating, defense: def_rating } }
    }

    req = Net::HTTP::Post.new("/spi", { 'Content-Type' => 'application/json' })
    req.body = Oj.dump(json_map, mode: :compat)
    response = Net::HTTP.new("localhost", 6577).start { |http| http.read_timeout = 300; http.request(req) }

    sql = "INSERT INTO teams (id,off_rating,def_rating,rating,created_at,updated_at) VALUES "
    sql2 = "INSERT INTO historical_ratings (team_id,off_rating,def_rating,rating,measure_date) VALUES "
    now = Time.zone.now.to_s.chop.chop.chop.chop
    Oj.load(response.body, bigdecimal_load: :float).each do |k, v|
      sql << "(#{k}, #{v ? v["Offense"] : "NULL"}, #{v ? v["Defense"] : "NULL"}, #{v ? v["Team"] : "NULL"}, '#{now}', '#{now}'),"
      if v != nil
        sql2 << "(#{k}, #{v["Offense"]}, #{v["Defense"]}, #{v["Team"]}, '#{Date.today.strftime(Date::DATE_FORMATS[:db])}'),"
      end
    end
    sql.chop!
    sql2.chop!
    sql << "ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at);"
    sql2 << "ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating);"
    ActiveRecord::Base.connection.execute(sql)
    ActiveRecord::Base.connection.execute(sql2)
  end

  def retrieve_geocode
    url = "https://nominatim.openstreetmap.org/search.php?q=#{CGI.escape(city.to_s + ", " + country)}&format=jsonv2&namedetails=1&layer=address"
    uri = URI(url)
    response = Net::HTTP.get(uri, {'User-Agent' => "GolAberto (www.golaberto.com)"})
    self.build_team_geocode unless self.team_geocode
    self.team_geocode.update(data: JSON.parse(response))
    return true
  end
end
