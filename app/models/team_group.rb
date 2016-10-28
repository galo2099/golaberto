class TeamGroup < ActiveRecord::Base
  serialize :odds
  belongs_to :group, :touch => true
  belongs_to :team
  validates_presence_of :group
  validates_presence_of :team
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

  def first_odds
    calculate_odds(0, 1)
  end

  def promoted_odds
    calculate_odds(0, group.promoted)
  end

  def relegated_odds
    calculate_odds(-group.relegated, group.relegated)
  end

  private
  def calculate_odds(start, num)
    odds.try(:slice, start, num).try(:sum)
  end

end
