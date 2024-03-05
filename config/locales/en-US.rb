{
  "en-US": {
    round: -> (_key, round:, **_options) {
      "#{round.ordinalize} Round"
    },
    "distance": -> (_key, distance:, **_options) {
      options = { precision: 0 }.merge(_options)
      "#{ApplicationController.helpers.number_with_precision(distance * 0.621371, precision: options[:precision])} miles"
    },
  }
}
