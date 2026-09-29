module OddsFormattingHelper
  # Odds in views are percentages (0..100), not fractions (0..1).
  def formatted_odds(percentage, compact: false)
    return "" if percentage.nil?

    value = percentage.to_f
    if value > 0 && value < 0.01
      mantissa, exponent = format("%.0e", value).split("e")
      return "#{mantissa}e#{exponent.to_i}#{compact ? '' : '%'}"
    end
    if value > 99.99 && value < 100
      formatted = ">#{number_to_percentage(99.99, precision: 2)}"
      return compact ? formatted.delete_suffix("%") : formatted
    end

    formatted = number_to_percentage(value, precision: 2)
    compact ? formatted.delete_suffix("%") : formatted
  end

  def odds_title(percentage)
    return nil if percentage.nil?

    value = percentage.to_f
    if value > 0 && value < 0.01
      mantissa, exponent = format("%.3e", value).split("e")
      separator = I18n.t("number.format.separator", default: ".")
      mantissa = mantissa.sub(/0+\z/, "").sub(/\.\z/, "").tr(".", separator)
      return "#{mantissa}e#{exponent.to_i}%"
    end

    if value >= 0.01 && value <= 99.99
      formatted = number_to_percentage(value, precision: 4, significant: true, strip_insignificant_zeros: true)
      return number_to_percentage(99.99, precision: 2) if formatted == number_to_percentage(100, precision: 4, significant: true)

      return formatted
    end

    number_to_percentage(value, precision: 16, strip_insignificant_zeros: true)
  end
end
