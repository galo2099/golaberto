class TeamPlayer < ApplicationRecord
  belongs_to :team
  belongs_to :player
  belongs_to :championship

  validates_uniqueness_of :team_id, :scope => [ :player_id, :championship_id ]

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: championship_id , SQL Definition:bigint(20)
end
