class Game < ActiveRecord::Base
  belongs_to :home, :class_name => "Team", :foreign_key => "home_id"
  belongs_to :away, :class_name => "Team", :foreign_key => "away_id"
  belongs_to :phase
  belongs_to :stadium
  belongs_to :referee

  has_many :goals,
           :order => :time,
           :include => :player,
           :dependent => :delete_all

  has_many :home_player_games,
           :class_name => "PlayerGame",
           :dependent => :delete_all,
           :conditions => 'team_id = #{home_id}',
           :include => :player,
           :order => "player_games.off DESC, player_games.on"

  has_many :away_player_games,
           :class_name => "PlayerGame",
           :dependent => :delete_all,
           :conditions => 'team_id = #{away_id}',
           :include => :player,
           :order => "player_games.off DESC, player_games.on"

  validates_presence_of :home
  validates_presence_of :away
  validates_presence_of :phase
  validates_presence_of :date
  validates_numericality_of :home_score, :only_integer => true
  validates_numericality_of :away_score, :only_integer => true
  validates_numericality_of :home_pen, :only_integer => true, :allow_nil => true
  validates_numericality_of :away_pen, :only_integer => true, :allow_nil => true

  def validate
    errors.add(:home, "can't play with itself") if home_id == away_id
  end

  def formatted_date
    unless date.nil?
      date.to_s(:date)
    end
  end

  def formatted_time
    unless date.nil? or (date.hour == 0 and date.min == 0)
      date.to_s(:time)
    end
  end

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: home_id , SQL Definition:bigint(20)
  # Field: away_id , SQL Definition:bigint(20)
  # Field: phase_id , SQL Definition:bigint(20)
  # Field: round , SQL Definition:tinyint(4)
  # Field: attendance , SQL Definition:mediumint(9)
  # Field: date , SQL Definition:datetime
  # Field: stadium_id , SQL Definition:bigint(20)
  # Field: referee_id , SQL Definition:bigint(20)
  # Field: home_score , SQL Definition:tinyint(2)
  # Field: away_score , SQL Definition:tinyint(2)
  # Field: home_pen , SQL Definition:tinyint(2)
  # Field: away_pen , SQL Definition:tinyint(2)
  # Field: played , SQL Definition:enum('scheduled','played','playing')


end
