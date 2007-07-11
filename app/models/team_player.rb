class TeamPlayer < ActiveRecord::Base
  belongs_to :team
  belongs_to :player
  belongs_to :championship

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)

end
