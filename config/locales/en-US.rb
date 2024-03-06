{
  "en-US": {
    round: -> (_key, round:, **_options) {
      "#{round.ordinalize} Round"
    },
    "height": -> (_key, height:, **_options) {
      return "" if height.nil?
      total_inches = height / 2.54
      feet = total_inches / 12
      inches = total_inches % 12

      # If inches calculation results in 12, adjust to 0 inches and add 1 to feet
      if inches.round == 12
        feet += 1
        inches = 0
      end

      "#{feet.floor}'#{inches.round}''"
    },
    "distance": -> (_key, distance:, **_options) {
      options = { precision: 0 }.merge(_options)
      "#{ApplicationController.helpers.number_with_precision(distance * 0.621371, precision: options[:precision])} miles"
    },
  }
}
