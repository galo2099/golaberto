{
  "pt-BR": {
    round: -> (_key, round:, **_options) {
      "#{I18n.t("number.nth.ordinalized", number: round, gender: :female)} Rodada"
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
