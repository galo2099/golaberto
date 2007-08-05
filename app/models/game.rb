class Game < ActiveRecord::Base
  belongs_to :home, :class_name => "Team", :foreign_key => "home_id"
  belongs_to :away, :class_name => "Team", :foreign_key => "away_id"
  belongs_to :phase
  belongs_to :stadium
  belongs_to :referee

  has_many :goals,
           :class_name => "Goal",
           :order => :time,
           :include => :player,
           :dependent => :delete_all

  has_many :home_goals,
           :class_name => "Goal",
           :order => :time,
           :include => :player,
           :conditions => '(team_id = #{home_id} and own_goal = "0") or (team_id = #{away_id} and own_goal = "1")'

  has_many :away_goals,
           :class_name => "Goal",
           :order => :time,
           :include => :player,
           :conditions => '(team_id = #{away_id} and own_goal = "0") or (team_id = #{home_id} and own_goal = "1")'

  has_many :player_games, :include => :player, :dependent => :delete_all

  has_many :home_player_games,
           :class_name => "PlayerGame",
           :conditions => 'team_id = #{home_id}',
           :include => :player

  has_many :away_player_games,
           :class_name => "PlayerGame",
           :conditions => 'team_id = #{away_id}',
           :include => :player

  validates_presence_of :home
  validates_presence_of :away
  validates_presence_of :phase
  validates_presence_of :date
  validates_inclusion_of :played, :in => [ true, false ]
  validates_numericality_of :home_score, :only_integer => true
  validates_numericality_of :away_score, :only_integer => true
  validates_numericality_of :home_pen, :only_integer => true, :allow_nil => true
  validates_numericality_of :away_pen, :only_integer => true, :allow_nil => true

  def validate
    errors.add(:home, "can't play with itself") if home_id == away_id
  end

  def played_str
    played? ? "Played" : "Scheduled"
  end

  def formatted_date(day = false)
    ret = ""
    unless date.nil?
      ret = date.strftime("%d/%m/%Y")
      ret << " - " << date.strftime("%A") if day
    end
    ret
  end

  def formatted_time
    unless time.nil?
      time.strftime("%H:%M")
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
  # Field: date , SQL Definition:date
  # Field: time , SQL Definition:time
  # Field: stadium_id , SQL Definition:bigint(20)
  # Field: referee_id , SQL Definition:bigint(20)
  # Field: home_score , SQL Definition:tinyint(2)
  # Field: away_score , SQL Definition:tinyint(2)
  # Field: home_pen , SQL Definition:tinyint(2)
  # Field: away_pen , SQL Definition:tinyint(2)
  # Field: played , SQL Definition:tinyint(1)


end
