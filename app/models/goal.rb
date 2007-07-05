class Goal < ActiveRecord::Base
  self.table_name = %( `Goal` )
  self.primary_key = "id"



  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player , SQL Definition:bigint(20)
  # Field: game , SQL Definition:bigint(20)
  # Field: team , SQL Definition:bigint(20)
  # Field: time , SQL Definition:tinyint(4)
  # Field: penalty , SQL Definition:enum('0','1')
  # Field: own_goal , SQL Definition:enum('0','1')


end
