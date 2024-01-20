require "active_support/concern"

module Country
  extend ActiveSupport::Concern

  included do
    scope :disabled, -> { where(disabled: true) }
  end

  class_methods do
    def small_country_flag(country)
      if country.nil?
        "#{Rails.configuration.golaberto_image_url_prefix}/thumb.png"
      else
        "#{Rails.configuration.golaberto_image_url_prefix}/countries/flags/#{country.parameterize(separator: '_')}_15.png"
      end
    end

    def large_country_flag(country)
      if country.nil?
        "#{Rails.configuration.golaberto_image_url_prefix}/thumb.png"
      else
        "#{Rails.configuration.golaberto_image_url_prefix}/countries/flags/#{country.parameterize(separator: '_')}_100.png"
      end
    end
  end
end

