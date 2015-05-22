Rails.application.config.middleware.use OmniAuth::Builder do
  provider :google_oauth2,
           Rails.application.secrets.google_api["client_id"],
           Rails.application.secrets.google_api["secret"],
           { name: "google",
             openid_realm: Rails.application.secrets.google_api["openid_realm"],
             access_type: "online",
             scope: "openid",
           }
end
