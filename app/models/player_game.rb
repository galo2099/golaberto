class PlayerGame < ActiveRecord::Base
  self.table_name = "player_game"
  self.primary_key = ""



  # Fields information, just FYI.
  #
  # Field: player , SQL Definition:bigint(20)
  # Field: game , SQL Definition:bigint(20)
  # Field: team , SQL Definition:bigint(20)
  # Field: on , SQL Definition:tinyint(2)
  # Field: off , SQL Definition:tinyint(2)
  # Field: yellow , SQL Definition:tinyint(2)
  # Field: red , SQL Definition:tinyint(2)


end
