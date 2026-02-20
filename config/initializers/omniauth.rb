OmniAuth.config.allowed_request_methods = [:get, :post]

google_api_credentials = Rails.application.credentials.google_api
google_client_id = google_api_credentials&.[](:client_id) || google_api_credentials&.[]("client_id")
google_secret = google_api_credentials&.[](:secret) || google_api_credentials&.[]("secret")

Rails.application.config.middleware.use OmniAuth::Builder do
  if google_client_id && google_secret && !google_client_id.empty? && !google_secret.empty?
    provider :google_oauth2,
             google_client_id,
             google_secret,
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
