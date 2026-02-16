class HistoricalOdds < ApplicationRecord
  serialize :odds, type: Array
  belongs_to :team_group

  validates :team_group_id, presence: true
  validates :measure_date, presence: true
  validates :measure_date, uniqueness: { scope: :team_group_id }

  def calculate_odds(positions)
    return nil if not odds or positions.nil?
    positions.map{|p| odds[p-1].to_f}.sum
  end
end
