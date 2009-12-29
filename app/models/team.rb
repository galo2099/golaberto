class Team < ActiveRecord::Base
  include ImageUpload

  belongs_to :stadium
  has_many :comments, :as => :commentable, :dependent => :destroy, :order => 'created_at ASC'
  has_many :team_groups, :dependent => :delete_all
  has_many :groups, :through => :team_groups
  has_many :home_games, :foreign_key => "home_id", :class_name => "Game", :dependent => :destroy
  has_many :away_games, :foreign_key => "away_id", :class_name => "Game", :dependent => :destroy
  has_many :team_players, :dependent => :delete_all, :include => :player
  validates_length_of :name, :within => 1..40
  validates_length_of :country, :within => 1..40
  validates_uniqueness_of :name, :message => "already exists"

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: country , SQL Definition:varchar(255)
  # Field: logo , SQL Definition:varchar(255)

  def to_param
    "#{id}-#{name.parameterize}"
  end

  def country_small_logo
    if country.nil?
      'logos/15.png'
    else
      "countries/#{country.parameterize('_')}_15.png"
    end
  end

  def country_large_logo
    if country.nil?
      'logos/100.png'
    else
      "countries/#{country.parameterize('_')}_100.png"
    end
  end

  def small_logo
    if logo.nil?
      '15.png'
    else
      logo.gsub(/(.*)\.svg/, '\1_15.png')
    end
  end

  def large_logo
    if logo.nil?
      '100.png'
    else
      logo.gsub(/(.*)\.svg/, '\1_100.png')
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
    ret = n_games(n, cond, "ASC")
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
    ret = n_games(n, cond, "DESC")
  end

  def uploaded_logo(l, filter_background = false)
    uploaded_image(l, "logos", filter_background)
    self.logo = "#{self.id}.svg"
    save!
  end

  private
  def n_games(n, condition, order)
    condition[0] << " AND (home_id = ? OR away_id = ?)"
    condition << id
    condition << id
    Game.find(
        :all,
        :order => "date #{order}",
        :conditions => condition,
        :include => [ { :phase => :championship } ],
        :limit => n)
  end

end
