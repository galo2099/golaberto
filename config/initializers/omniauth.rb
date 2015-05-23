Rails.application.config.middleware.use OmniAuth::Builder do
  provider :google_oauth2,
           Rails.application.secrets.google_api["client_id"],
           Rails.application.secrets.google_api["secret"],
           { name: "google",
             access_type: "online",
             scope: "openid",
             setup: lambda do |env|
               request = Rack::Request.new(env)
               env['omniauth.strategy'].options[:openid_realm] = request.base_url
             end,
           }
end
