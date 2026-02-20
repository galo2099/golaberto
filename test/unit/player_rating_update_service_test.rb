require "test_helper"

class PlayerRatingUpdateServiceTest < ActiveSupport::TestCase
  class FakeHttp
    attr_accessor :read_timeout
    attr_reader :request

    def initialize(response)
      @response = response
    end

    def start
      yield self
    end

    def request(req)
      @request = req
      @response
    end
  end

  test "posts to player ratings endpoint with configured timeout" do
    response = Net::HTTPOK.new("1.1", "200", "OK")
    fake_http = FakeHttp.new(response)
    host = nil
    port = nil

    actual_response = Net::HTTP.stub(:new, lambda { |h, p|
      host = h
      port = p
      fake_http
    }) do
      PlayerRatingUpdateService.run
    end

    assert_equal response, actual_response
    assert_equal PlayerRatingUpdateService::HOST, host
    assert_equal PlayerRatingUpdateService::PORT, port
    assert_equal PlayerRatingUpdateService::READ_TIMEOUT_SECONDS, fake_http.read_timeout
    assert_equal PlayerRatingUpdateService::PATH, fake_http.request.path
    assert_equal "application/json", fake_http.request["Content-Type"]
  end
end
