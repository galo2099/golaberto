class Player < ActiveRecord::Base
  has_many :goals, :dependent => :destroy

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: position , SQL Definition:set('g','dr','dl','dc','dm','cm','am','fw')
  # Field: birth , SQL Definition:date
  # Field: country , SQL Definition:varchar(255)
  # Field: full_name , SQL Definition:varchar(255)


end
