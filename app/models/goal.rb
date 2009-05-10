class Goal < ActiveRecord::Base
  belongs_to :player
  belongs_to :game
  belongs_to :team

  validates_presence_of :player
  validates_presence_of :game
  validates_presence_of :team
  validates_numericality_of :time, :only_integer => true
  validates_inclusion_of :penalty, :in => [ true, false ]
  validates_inclusion_of :own_goal, :in => [ true, false ]

  named_scope :regulation, :conditions => { :aet => 0 }
  named_scope :aet, :conditions => { :aet => 1 }

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: game_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: time , SQL Definition:tinyint(4)
  # Field: penalty , SQL Definition:tinyint(1)
  # Field: own_goal , SQL Definition:tinyint(1)


end
