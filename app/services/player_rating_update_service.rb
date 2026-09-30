require "net/http"

class PlayerRatingUpdateService
  HOST = "localhost".freeze
  PORT = 6577
  PATH = "/player_ratings".freeze
  READ_TIMEOUT_SECONDS = 300

  def self.run
    req = Net::HTTP::Post.new(PATH, { "Content-Type" => "application/json" })
    Net::HTTP.new(HOST, PORT).start do |http|
      http.read_timeout = READ_TIMEOUT_SECONDS
      response = http.request(req)
      response.value
      response
    end
  end
end
