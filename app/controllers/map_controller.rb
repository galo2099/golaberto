require 'net/http'
require 'uri'
require 'google_url_signer'

class MapController < ApplicationController
  skip_authorization_check
  GOOGLE_STATIC_MAPS_API_KEY = 'AIzaSyCT_RQIGXyWC6LEKwGVkiIAyXJjWfuKJkE'

  def static
    query = {
      markers: normalize_markers(params[:markers]),
      path: params[:path],
      zoom: params[:zoom],
      region: params[:region],
      language: params[:language],
      size: params[:size] || '560x400',
      scale: params[:scale] || '2',
      format: params[:format] || 'jpg',
      maptype: params[:maptype],
      key: GOOGLE_STATIC_MAPS_API_KEY
    }.compact

    base_url = "https://maps.googleapis.com/maps/api/staticmap?#{URI.encode_www_form(query)}"
    signed_url = GoogleUrlSigner.sign(base_url, Rails.application.credentials.google_api[:secret_sign])
    uri = URI.parse(signed_url)

    response = Net::HTTP.start(uri.host, uri.port, use_ssl: true) do |http|
      http.open_timeout = 5
      http.read_timeout = 10
      http.get(uri.request_uri)
    end

    log_static_map_warnings(response)

    if response.is_a?(Net::HTTPSuccess)
      send_data response.body,
        type: response['content-type'] || 'image/jpeg',
        disposition: 'inline'
    else
      head :bad_gateway
    end
  end

  private

  def normalize_markers(markers)
    return if markers.blank?

    marker_pairs = if markers.respond_to?(:to_unsafe_h)
      markers.to_unsafe_h.values
    elsif markers.is_a?(Hash)
      markers.values
    else
      Array(markers)
    end

    marker_pairs.filter_map do |marker|
      style, coords = marker
      style = style.to_s
      coords = coords.to_s
      next if style.blank? || coords.blank?

      "#{style}|#{coords}"
    end.presence
  end

  def log_static_map_warnings(response)
    warning_header = response['X-Staticmap-API-Warning']
    return if warning_header.blank?

    warning_header.split(',').each do |warning|
      Rails.logger.warn("Google Static Maps warning: #{warning.strip}")
    end
  end
end
