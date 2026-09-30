require "net/http"
require_relative "../../app/services/player_rating_update_service"

class PlayerRatingUpdateServiceTest < Minitest::Test
  class FakeHttp
    attr_accessor :read_timeout
    attr_reader :last_request

    def initialize(response)
      @response = response
    end

    def start
      yield self
    end

    def request(req)
      @last_request = req
      @response
    end
  end

  def test_posts_to_player_ratings_endpoint_with_configured_timeout
    response = Net::HTTPOK.new("1.1", "200", "OK")
    fake_http = FakeHttp.new(response)
    host = nil
    port = nil

    original_new = Net::HTTP.method(:new)
    begin
      Net::HTTP.singleton_class.send(:define_method, :new) do |h, p|
        host = h
        port = p
        fake_http
      end
      actual_response = PlayerRatingUpdateService.run
    ensure
      Net::HTTP.singleton_class.send(:define_method, :new, original_new)
    end

    assert_equal response, actual_response
    assert_equal PlayerRatingUpdateService::HOST, host
    assert_equal 6577, port
    assert_equal PlayerRatingUpdateService::READ_TIMEOUT_SECONDS, fake_http.read_timeout
    assert_equal PlayerRatingUpdateService::PATH, fake_http.last_request.path
    assert_equal "application/json", fake_http.last_request["Content-Type"]
  end

  def test_failed_update_is_raised_instead_of_reported_as_success
    response = Net::HTTPInternalServerError.new("1.1", "500", "Internal Server Error")
    fake_http = FakeHttp.new(response)
    original_new = Net::HTTP.method(:new)
    begin
      Net::HTTP.singleton_class.send(:define_method, :new) { |*| fake_http }
      assert_raises(Net::HTTPFatalError) { PlayerRatingUpdateService.run }
    ensure
      Net::HTTP.singleton_class.send(:define_method, :new, original_new)
    end
  end
end
