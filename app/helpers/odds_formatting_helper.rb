require "bigdecimal"

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
    if value > 0 && value < 0.01
      _, digits, _, exponent = BigDecimal(value.to_s).split
      rest = digits[1..].to_s.sub(/0+\z/, "")
      separator = I18n.t("number.format.separator", default: ".")
      mantissa = digits[0] + (rest.empty? ? "" : "#{separator}#{rest}")
      return "#{mantissa}e#{exponent - 1}%"
    end

    number_to_percentage(value, precision: 16, strip_insignificant_zeros: true)
  end
end
