OmniAuth.config.allowed_request_methods = [:get, :post]

google_api = Rails.application.credentials.google_api || {}
client_id = google_api[:client_id]
client_secret = google_api[:secret]

if client_id.present? && client_secret.present?
  Rails.application.config.middleware.use OmniAuth::Builder do
    provider :google_oauth2,
             client_id,
             client_secret,
             { name: "google",
               access_type: "online",
               scope: "openid",
               setup: lambda do |env|
                 request = Rack::Request.new(env)
                 env['omniauth.strategy'].options[:openid_realm] = request.base_url
               end,
             }
  end
end
