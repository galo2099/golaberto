require 'net/http'
require 'uri'
require 'google_url_signer'

class MapController < ApplicationController
  skip_authorization_check

  def static
    query = {
      markers: params[:markers],
      path: params[:path],
      zoom: params[:zoom],
      region: params[:region],
      language: params[:language],
      size: params[:size] || '560x400',
      scale: params[:scale] || '2',
      format: params[:format] || 'jpg',
      maptype: params[:maptype],
      key: Rails.application.credentials.google_api[:api_key]
    }.compact

    base_url = "https://maps.googleapis.com/maps/api/staticmap?#{URI.encode_www_form(query)}"
    signed_url = GoogleUrlSigner.sign(base_url, Rails.application.credentials.google_api[:secret_sign])
    uri = URI.parse(signed_url)

    response = Net::HTTP.start(uri.host, uri.port, use_ssl: true) do |http|
      http.open_timeout = 5
      http.read_timeout = 10
      http.get(uri.request_uri)
    end

    if response.is_a?(Net::HTTPSuccess)
      send_data response.body,
        type: response['content-type'] || 'image/jpeg',
        disposition: 'inline'
    else
      head :bad_gateway
    end
  end
end
