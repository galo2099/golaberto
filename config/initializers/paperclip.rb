Paperclip.interpolates :bucket  do |attachment, style|
  credentials = Rails.application.credentials.s3
  if credentials.respond_to?(:[])
    credentials[:bucket] || credentials['bucket'] || ENV['S3_BUCKET'] || 'golaberto-local'
  else
    ENV['S3_BUCKET'] || 'golaberto-local'
  end
end
