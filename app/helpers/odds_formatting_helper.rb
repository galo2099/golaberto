module OddsFormattingHelper
  # Odds in views are percentages (0..100), not fractions (0..1).
  def formatted_odds(percentage)
    return "" if percentage.nil?

    value = percentage.to_f
    if value > 0 && value < 0.01
      mantissa, exponent = format("%.0e", value).split("e")
      return "#{mantissa}e#{exponent.to_i}%"
    end
    if value > 99.99 && value < 100
      return ">#{number_to_percentage(99.99, precision: 2)}"
    end

    number_to_percentage(value, precision: 2)
  end

  # The standings table uses probabilities on a 0..1 scale with a decimal dot.
  def formatted_odds_fraction(percentage)
    return "" if percentage.nil?

    value = percentage.to_f
    return "0" if value.zero?
    return "1" if value == 100

    probability = value / 100
    if probability > 0 && probability < 0.0001
      mantissa, exponent = format("%.0e", probability).split("e")
      return "#{mantissa}e#{exponent.to_i}"
    end
    if probability > 0.9999 && probability < 1
      mantissa, exponent = format("%.0e", (100 - value) / 100).split("e")
      return "1-#{mantissa}e#{exponent.to_i}"
    end

    format("%.4f", probability).sub(/0+\z/, "").sub(/\.\z/, "").sub(/\A0\./, ".")
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
