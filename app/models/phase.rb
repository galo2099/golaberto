class Phase < ApplicationRecord
  belongs_to :championship, :touch => true
  has_many   :groups, ->{ order :id }, :dependent => :destroy
  has_many   :team_groups, :through => :groups
  has_many   :teams, ->{ order :name }, :through => :team_groups
  has_many   :games, ->{ order :date }, :dependent => :destroy
  has_many   :goals, :through => :games
  validates_length_of :name, :within => 1..40
  validates_uniqueness_of :name, :scope => :championship_id
  validates_numericality_of :order_by, :only_integer => true
  validates_presence_of :sort
  validates_presence_of :championship, :message => "doesn't exist"
  validates_uniqueness_of :order_by, :scope => :championship_id

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: order_by , SQL Definition:tinyint(4)
  # Field: sort , SQL Definition:varchar(255)

  def self.sort_options
    { "pt" => _("points"),
      "w" => _("wins"),
      "gd" => _("goal difference"),
      "gf" => _("goals for"),
      "name" => _("name"),
      "g_average" => _("goal average"),
      "gp" => _("penalty goals"),
      "g_away" => _("goals away"),
      "bias" => _("bias"),
      "g_aet" => _("aet goals"),
      "head" => _("head to head") }
  end

  def to_param
    "#{id}-#{name.parameterize}"
  end

  def avg_team_rating
    #@avg_team_rating ||= begin teams.map{|t|t.rating.to_f}.sum / teams.size rescue 0 end
    @avg_team_rating ||= begin teams.sort_by{|t|-t.rating.to_f}.each_with_index.map{|t,i|t.rating.to_f * (0.7)**i}.sum / teams.each_with_index.map{|t,i| 0.7**i}.sum rescue 0 end
    #@avg_team_rating ||= contra_harmonic_mean(teams.map{|t|t.rating.to_f})
  end

  def contra_harmonic_mean(x)
    x.map{|x| x.to_f*x.to_f}.sum / x.map{|x|x.to_f}.sum
  end
end
