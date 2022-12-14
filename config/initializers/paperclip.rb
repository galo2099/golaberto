Paperclip.interpolates :bucket  do |attachment, style|
  Rails.application.secrets.s3[:bucket]
end

