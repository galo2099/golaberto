{
  "pt-BR": {
    round: -> (_key, round:, **_options) {
      "#{I18n.t("number.nth.ordinalized", number: round, gender: :female)} Rodada"
    },
    "height": -> (_key, height:, **_options) {
      return "" if height.nil?
      "#{height} cm"
    },
    "distance": -> (_key, distance:, **_options) {
      options = { precision: 0 }.merge(_options)
      "#{ApplicationController.helpers.number_with_precision(distance, precision: options[:precision])} Km"
    },
    number: {
      nth: {
        ordinals: -> (_key, number:, **_options) {
          if _options[:gender] == :female then
            "ª"
          else
            "º"
          end
        },
        ordinalized: -> (_key, number:, **_options) {
          "#{number}#{I18n.t("number.nth.ordinals", **_options.merge(number: number))}"
        },
      },
    },
  }
}
