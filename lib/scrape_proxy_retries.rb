# encoding: utf-8

SCRAPE_PUBLIC_PROXY_SOURCES = [
  'https://api.proxyscrape.com/v4/free-proxy-list/get?request=display_proxies&proxy_format=protocolipport&format=text&protocol=http&timeout=5000',
  'https://cdn.jsdelivr.net/gh/proxifly/free-proxy-list@main/proxies/protocols/http/data.txt',
].freeze unless defined?(SCRAPE_PUBLIC_PROXY_SOURCES)

SCRAPE_PUBLIC_PROXY_CACHE_SECONDS = 10 * 60 unless defined?(SCRAPE_PUBLIC_PROXY_CACHE_SECONDS)

def sofascore_http_options(url, proxy = nil)
  options = {
    cookies: { 'OptanonConsent': 'isGpcEnabled' },
    timeout: 20,
    headers: {
      "accept": "text/javascript, text/html, application/xml, text/xml, */*",
      "accept-language": "accept-language: en-US,en;q=0.9",
      "cache-control": "no-cache",
      "pragma": "no-cache",
      "priority": "u=1, i",
      "referer": url,
      "user-agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
      "authority" => "www.sofascore.com",
      "accept-encoding" => "deflate, gzip",
      'sec-ch-ua' => '"Not)A;Brand";v="99", "Google Chrome";v="127", "Chromium";v="127"',
      'sec-ch-ua-mobile' => '?0',
      'sec-ch-ua-platform' => '"macOS"',
      'sec-fetch-dest' => 'document',
      'sec-fetch-mode' => 'navigate',
      "connection" => "",
    },
  }

  if proxy
    host, port = proxy.sub(/\Ahttps?:\/\//, '').split(':', 2)
    options[:http_proxyaddr] = host
    options[:http_proxyport] = port.to_i
  end

  options
end

def fetch_public_proxies
  @public_proxies_fetched_at ||= Time.at(0)
  @public_proxies ||= []

  if @public_proxies.any? && Time.now - @public_proxies_fetched_at < SCRAPE_PUBLIC_PROXY_CACHE_SECONDS
    return @public_proxies
  end

  proxies = SCRAPE_PUBLIC_PROXY_SOURCES.flat_map do |source|
    Rails.logger.info "Loading public proxies from #{source}"
    HTTParty.get(source, timeout: 10).body.to_s.lines.map(&:strip)
  rescue StandardError => e
    Rails.logger.info "Cannot load proxy list from #{source}: #{e.class}: #{e.message}"
    []
  end

  @public_proxies = proxies
    .reject(&:empty?)
    .map { |proxy| proxy.start_with?('http://', 'https://') ? proxy : "http://#{proxy}" }
    .select { |proxy| proxy.match?(%r{\Ahttps?://[^\s:/]+:\d+\z}) }
    .uniq
    .shuffle
  @public_proxies_fetched_at = Time.now

  @public_proxies
end

def temporary_http_failure?(response, body)
  code = response.respond_to?(:code) ? response.code.to_i : 0
  return true if code == 0 || code == 403 || code == 429 || code >= 500
  return true if body.include?("Service Unavailable")
  return true if body.include?("Internal Server Error")
  return true if body.include?("Too Many Requests")

  false
end

def with_http_retries(url)
  loop do
    proxies = fetch_public_proxies

    if proxies.empty?
      Rails.logger.info "No public proxies available for [#{url}]. Retrying in 5 seconds."
      sleep 5
      next
    end

    proxies.each do |proxy|
      begin
        Rails.logger.info "Trying proxy #{proxy} for [#{url}]"
        response = HTTParty.get(url, sofascore_http_options(url, proxy))
        body = response.body.to_s

        return body unless temporary_http_failure?(response, body)

        Rails.logger.info "Proxy #{proxy} returned HTTP #{response.code} for [#{url}]. Trying next proxy."
      rescue StandardError => e
        Rails.logger.info "Proxy #{proxy} failed for [#{url}]: #{e.class}: #{e.message}. Trying next proxy."
      end
    end

    @public_proxies = []
    Rails.logger.info "All proxies failed for [#{url}]. Refreshing proxy list in 5 seconds."
    sleep 5
  end
end
