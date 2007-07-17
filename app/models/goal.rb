class Goal < ActiveRecord::Base
  belongs_to :player
  belongs_to :game
  belongs_to :team

  validates_presence_of :player
  validates_presence_of :game
  validates_presence_of :team
  validates_presence_of :time
  validates_presence_of :penalty
  validates_presence_of :goal

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
