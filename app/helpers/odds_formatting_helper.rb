require 'bigdecimal'

module OddsFormattingHelper
  def odds_background_color(percentage, maximum)
    strength = odds_color_strength(percentage, maximum)
    return "lightgray" if strength <= 0
    return "dimgray" if strength >= 100

    shade = (211 - 106 * strength / 100).round
    "rgb(#{shade}, #{shade}, #{shade})"
  end

  def odds_text_color(percentage, maximum)
    odds_color_strength(percentage, maximum) >= 65 ? "white" : "inherit"
  end

  def odds_color_strength(percentage, maximum)
    return 0 if percentage.nil? || maximum.to_f <= 0

    ratio = [[percentage.to_f / maximum.to_f, 0].max, 1].min
    Math.sqrt(ratio) * 100
  end

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
    return ".0000" if value.zero?
    return "1.000" if value == 100

    probability = value / 100
    if probability > 0 && probability < 0.0001
      mantissa, exponent = format("%.0e", probability).split("e")
      return "#{mantissa}e#{exponent.to_i}"
    end
    if probability > 0.9999 && probability < 1
      mantissa, exponent = format("%.0e", (100 - value) / 100).split("e")
      return "1-#{mantissa}e#{exponent.to_i}"
    end

    format("%.4f", probability).sub(/\A0\./, ".")
  end

  def compact_team_odds_parts(percentage)
    return nil if percentage.nil?

    value = BigDecimal(percentage.to_s)
    return nil unless value.finite? && value >= 0 && value <= 100
    return { integer: '0', first: '0', second: '' } if value.zero?
    return { integer: '100', first: '0', second: '' } if value == 100

    rounded = value.round(2, BigDecimal::ROUND_HALF_UP)
    if rounded > 0 && rounded < 100
      integer, fraction = rounded.to_s('F').split('.', 2)
      digits = (fraction || '').ljust(2, '0')[0, 2]
      return { integer: integer, first: digits[0], second: digits[1] }
    end

    if rounded.zero?
      significant = value.round(1 - value.exponent, BigDecimal::ROUND_HALF_UP)
      _, digits, _, exponent = significant.split
      count = -exponent
      return { fallback: "#{digits[0]}e#{exponent - 1}" } if count > 99

      return { integer: '0', first: compact_odds_count(count), second: digits[0], count: :first }
    end

    gap = BigDecimal('100') - value
    # Rounding a halfway gap down rounds the displayed percentage upward.
    significant = gap.round(1 - gap.exponent, BigDecimal::ROUND_HALF_DOWN)
    _, digits, _, exponent = significant.split
    count = -exponent
    return { fallback: "100 − #{digits[0]}e#{exponent - 1}" } if count > 99

    { integer: '99', first: compact_odds_count(count), second: (10 - digits[0].to_i).to_s, count: :first }
  end

  def compact_team_odds_text(percentage)
    parts = compact_team_odds_parts(percentage)
    return '' if parts.nil?

    parts[:fallback] || "#{parts[:integer]}.#{parts[:first]}#{parts[:second]}"
  end

  def formatted_compact_team_odds(percentage)
    parts = compact_team_odds_parts(percentage)
    return '' if parts.nil?
    return content_tag(:span, parts[:fallback], class: 'odds-compact-fallback') if parts[:fallback]

    fraction = [:first, :second].map do |slot|
      text = parts[slot]
      if parts[:count] == slot
        visible_count = text[1, 2].to_i.to_s
        count_class = 'odds-compact-count-digit'
        count_class += ' odds-compact-count-two-digits' if visible_count.length == 2
        count = content_tag(:span, visible_count, class: count_class, 'aria-hidden': true)
        dots = content_tag(:span, '..', class: 'odds-compact-count-dots', 'aria-hidden': true)
        text = content_tag(:span, count + dots, class: 'odds-compact-count', 'aria-label': text)
      end
      content_tag(:span, text, class: 'odds-compact-slot')
    end
    content_tag(:span, class: 'odds-compact-number') do
      content_tag(:span, parts[:integer], class: 'odds-compact-integer') +
        content_tag(:span, '.', class: 'odds-compact-dot') +
        content_tag(:span, fraction.join.html_safe, class: 'odds-compact-fraction')
    end
  end

  def compact_team_odds_title(percentage)
    return nil if percentage.nil?

    "#{percentage}%"
  end

  def compact_odds_count(count)
    "[#{count.to_s.rjust(2, '0')}]"
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
