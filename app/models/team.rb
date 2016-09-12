class Team < ActiveRecord::Base

  has_attached_file :logo,
      styles: lambda { |attachment|
        options = { format: "png", filter_background: attachment.instance.filter_image_background? }
        { medium: options.merge(geometry: "100x100"),
          thumb: options.merge(geometry: "15x15") }
      },
      processors: [ :logo ],
      storage: :s3,
      s3_credentials: Rails.application.secrets.s3,
      s3_headers: { 'Cache-Control' => 'max-age=315576000', 'Expires' => 10.years.from_now.httpdate },
      default_url: ActionController::Base.helpers.asset_path("blank.gif"), 
      path: ":class/:attachment/:id/:style.:extension"

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
      '/images/logos/15.png'
    else
      "/images/countries/#{country.parameterize('_')}_15.png"
    end
  end

  def large_country_logo
    if country.nil?
      '/images/logos/100.png'
    else
      "/images/countries/#{country.parameterize('_')}_100.png"
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
    ret = n_games(n, cond, :desc)
  end

  private
  def n_games(n, condition, order)
    condition[0] << " AND (home_id = ? OR away_id = ?)"
    condition << id
    condition << id
    Game.where(condition).order(date: order).includes({ :phase => :championship }).limit(n).references(:championship).to_a
  end

end
