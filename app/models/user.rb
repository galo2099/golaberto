class User < ActiveRecord::Base
  self.table_name = %( `User` )
  self.primary_key = "login"



  # Fields information, just FYI.
  #
  # Field: login , SQL Definition:varchar(30)
  # Field: pass , SQL Definition:varchar(50)


end
