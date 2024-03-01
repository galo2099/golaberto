require 'base64'
require 'openssl'
require 'uri'

module GoogleUrlSigner
  class << self
    def sign(url, key)
      parsed_url = URI.parse(url)
      full_path = "#{parsed_url.path}?#{parsed_url.query}"

      signature = generate_signature(full_path, key)

      "#{parsed_url.scheme}://#{parsed_url.host}#{full_path}&signature=#{signature}"
    end

    private

    def generate_signature(path, key)
      raw_key = url_safe_base64_decode(key)
      raw_signature = encrypt(raw_key, path)
      url_safe_base64_encode(raw_signature)
    end

    def encrypt(key, data)
      digest = OpenSSL::Digest.new('sha1')
      OpenSSL::HMAC.digest(digest, key, data)
    end

    def url_safe_base64_decode(base64_string)
      Base64.decode64(base64_string.tr('-_', '+/'))
    end

    def url_safe_base64_encode(raw)
      Base64.encode64(raw).tr('+/', '-_').strip
    end
  end
end
