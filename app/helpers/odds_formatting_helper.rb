module OddsFormattingHelper
  # Odds in views are percentages (0..100), not fractions (0..1).
  def formatted_odds(percentage)
    return "" if percentage.nil?

    value = percentage.to_f
    return "<#{number_to_percentage(0.01, precision: 2)}" if value > 0 && value < 0.01
    return ">#{number_to_percentage(99.99, precision: 2)}" if value > 99.99 && value < 100

    number_to_percentage(value, precision: 2)
  end

  def odds_title(percentage)
    return nil if percentage.nil?

    value = percentage.to_f
    formatted = number_to_percentage(value, precision: 16, strip_insignificant_zeros: true)
    return format("%.16g%%", value) if value != 0 && formatted.start_with?("0%")

    formatted
  end
end
