class Referee < ActiveRecord::Base
  self.table_name = %( `Referee` )
  self.primary_key = "id"



  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: location , SQL Definition:varchar(255)


end
