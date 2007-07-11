class PlayerGame < ActiveRecord::Base
  belongs_to :player
  belongs_to :game
  belongs_to :team

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: game_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: on , SQL Definition:tinyint(2)
  # Field: off , SQL Definition:tinyint(2)
  # Field: yellow , SQL Definition:tinyint(2)
  # Field: red , SQL Definition:tinyint(2)


end
