class Phase < ActiveRecord::Base
  belongs_to :championship
  has_many   :groups
  has_many   :games
  has_many   :teams, :finder_sql => 'SELECT teams.* FROM teams INNER JOIN team_groups ON (teams.id=team_groups.team_id) INNER JOIN groups ON (team_groups.group_id=groups.id) WHERE (groups.phase_id = #{id})'


  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: order_by , SQL Definition:tinyint(4)
  # Field: sort , SQL Definition:varchar(255)


end
