class Goal < ActiveRecord::Base
  belongs_to :player
  belongs_to :game
  belongs_to :team

  validates_presence_of :player
  validates_presence_of :game
  validates_presence_of :team

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: game_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: time , SQL Definition:tinyint(4)
  # Field: penalty , SQL Definition:enum('0','1')
  # Field: own_goal , SQL Definition:enum('0','1')


end
