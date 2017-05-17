require 'poisson'
class Team < ActiveRecord::Base

  AVG_BASE = 1.3350257653834494

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

  belongs_to :stadium
  has_many :comments, ->{ order(created_at: :asc) }, :as => :commentable, :dependent => :destroy
  has_many :team_groups, :dependent => :delete_all
  has_many :groups, :through => :team_groups
  has_many :home_games, :foreign_key => "home_id", :class_name => "Game", :dependent => :destroy
  has_many :away_games, :foreign_key => "away_id", :class_name => "Game", :dependent => :destroy
  has_many :team_players, ->{ includes :player}, :dependent => :delete_all
  validates_length_of :name, :within => 1..40
  validates_length_of :country, :within => 1..40
  validates_uniqueness_of :name, :message => "already exists"

  before_save :calculate_rating

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: country , SQL Definition:varchar(255)
  # Field: logo , SQL Definition:varchar(255)

  def filter_image_background?
    return filter_image_background == "1"
  end

  def to_param
    "#{id}-#{name.parameterize}"
  end

  def small_country_logo
    if country.nil?
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3["bucket"]}/thumb.png"
    else
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3["bucket"]}/countries/flags/#{country.parameterize('_')}_15.png"
    end
  end

  def large_country_logo
    if country.nil?
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3["bucket"]}/medium.png"
    else
      "https://s3.amazonaws.com/#{Rails.application.secrets.s3["bucket"]}/countries/flags/#{country.parameterize('_')}_100.png"
    end
  end

  def next_n_games(n, options = {})
    cond = [ "played = ?", false ]
    if options[:phase].nil?
      cond[0] << " AND championships.category_id = ?";
      cond << Category::DEFAULT_CATEGORY
    else
      cond[0] << " AND phase_id = ?"
      cond << phase.id
    end
    if options[:date]
      cond[0] << " AND date >= ?"
      cond << options[:date]
    end
    n_games(n, cond, :asc)
  end

  def last_n_games(n, options = {})
    cond = [ "played = ?", true ]
    if options[:phase].nil?
      cond[0] << " AND championships.category_id = ?";
      cond << Category::DEFAULT_CATEGORY
    else
      cond[0] << " AND phase_id = ?"
      cond << options[:phase].id
    end
    if options[:date]
      cond[0] << " AND date <= ?"
      cond << options[:date]
    end
    n_games(n, cond, :desc)
  end

  def self.calculate_rating2(off, deff)
    ten_array = (0...20).to_a
    home_power = Poisson.new([0.01, (off - AVG_BASE)/(AVG_BASE*0.424+0.548)*([0.25, (AVG_BASE)*0.424+0.548].max)+(AVG_BASE)].max)
    away_power = Poisson.new([0.01, deff].max)
    probs = ten_array.map{|i| ten_array.map{|j| home_power.p(i) * away_power.p(j) }}
    (3 * ten_array.map{|i| (0...i).to_a.map{|j| probs[i][j]}}.flatten.sum + ten_array.map{|i| probs[i][i]}.sum) / 3 * 100
  end

  private
  def n_games(n, condition, order)
    condition[0] << " AND (home_id = ? OR away_id = ?)"
    condition << id
    condition << id
    Game.where(condition).order(date: order).includes({ :phase => :championship }).limit(n).references(:championship).to_a
  end

  def calculate_rating
    self.rating = nil
    if off_rating && def_rating
      self.rating = calculate_rating2(off_rating, def_rating)
    end
  end
end
