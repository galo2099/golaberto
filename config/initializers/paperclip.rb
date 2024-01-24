Paperclip.interpolates :bucket  do |attachment, style|
  Rails.application.credentials.s3[:bucket]
end

