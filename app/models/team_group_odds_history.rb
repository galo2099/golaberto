class TeamGroupOddsHistory < ApplicationRecord
  serialize :odds

  belongs_to :team_group

  validates :recorded_on, presence: true
  validates :captured_at, presence: true
  validates :odds, presence: true
  validates :team_group_id, uniqueness: { scope: :recorded_on }

  def self.encode_odds(odds, statuses = nil)
    odds.each_with_index.map do |value, index|
      if value.is_a?(Numeric) && value.zero?
        status = statuses && statuses[index]
        status = "undecided" unless %w[impossible reachable undecided].include?(status)
        [value, status]
      else
        [value]
      end
    end
  end

  def numeric_odds
    return nil if odds.nil?

    Array(odds).map { |entry| entry.is_a?(Array) ? entry.first : entry }
  end

  def odds_reachability
    return nil if odds.nil?

    Array(odds).map do |entry|
      value = entry.is_a?(Array) ? entry.first : entry
      status = entry.is_a?(Array) ? entry[1] : nil
      next "reachable" if value.is_a?(Numeric) && value > 0
      next status if %w[impossible reachable undecided].include?(status)

      "undecided"
    end
  end
end
