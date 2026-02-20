class Goal < ApplicationRecord
  belongs_to :player
  belongs_to :game
  belongs_to :team

  validates_numericality_of :time, :only_integer => true
  validates_inclusion_of :penalty, :in => [ true, false ]
  validates_inclusion_of :own_goal, :in => [ true, false ]

  scope :regulation, ->{ where(:aet => 0) }
  scope :aet, ->{ where(:aet => 1) }

  def penalty=(value)
    super(normalize_legacy_boolean(value))
  end

  def own_goal=(value)
    super(normalize_legacy_boolean(value))
  end

  private

    def normalize_legacy_boolean(value)
      return nil if value.nil?

      value == true || value.to_s == '1'
    end

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: player_id , SQL Definition:bigint(20)
  # Field: game_id , SQL Definition:bigint(20)
  # Field: team_id , SQL Definition:bigint(20)
  # Field: time , SQL Definition:tinyint(4)
  # Field: penalty , SQL Definition:tinyint(1)
  # Field: own_goal , SQL Definition:tinyint(1)
end
