module CompactOdds
  extend ActiveSupport::Concern

  class_methods do
    def encode_odds(odds, statuses = nil)
      return nil if odds.nil?

      odds.each_with_index.map do |value, index|
        next value unless value.is_a?(Numeric) && value.zero?

        status = statuses && statuses[index]
        %w[reachable impossible].include?(status) ? [value, status] : value
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
      next status if %w[reachable impossible].include?(status)

      "undecided"
    end
  end
end
