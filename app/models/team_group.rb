class TeamGroup < ActiveRecord::Base
  serialize :odds
  belongs_to :group, :touch => true
  belongs_to :team
  validates_numericality_of :add_sub, :only_integer => true
  validates_numericality_of :bias, :only_integer => true
  validates_uniqueness_of :team_id, :scope => :group_id

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: group_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: add_sub , SQL Definition:int(4)
  # Field: bias , SQL Definition:tinyint(4)
  # Field: comment , SQL Definition:text

  def calculate_odds(positions)
    return nil if not odds
    positions.map{|p| odds[p-1]}.sum
  end
end
