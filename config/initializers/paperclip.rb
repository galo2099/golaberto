Paperclip.interpolates :bucket do |_attachment, _style|
  configured_bucket = Rails.application.config.paperclip_defaults.dig(:s3_credentials, :bucket) ||
                      Rails.application.config.paperclip_defaults.dig(:s3_credentials, 'bucket')
  configured_bucket.presence || 'golaberto_development'
end

