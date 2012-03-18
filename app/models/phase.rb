class Phase < ActiveRecord::Base
  belongs_to :championship, :touch => true
  has_many   :groups, :dependent => :destroy, :order => :id
  has_many   :team_groups, :through => :groups
  has_many   :teams, :through => :team_groups
  has_many   :games, :dependent => :destroy
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
end
