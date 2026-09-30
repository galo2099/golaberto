require_relative "../test_helper"

class HistoricalRatingEndpointTest < Minitest::Test
  class EmptyRelation
    def select(*) = self
    def where(*) = self
    def reorder(*) = self
    def pluck(*) = []
  end

  class FakeHttp
    attr_accessor :read_timeout
    attr_reader :request_sent

    def initialize(response)
      @response = response
    end

    def start
      yield self
    end

    def request(request)
      @request_sent = request
      @response
    end
  end

  def test_persisted_empty_series_redirects_without_another_database_write
    response = Struct.new(:body) do
      def value = self
    end.new('{"ratings":{},"offense":{},"defense":{},"dates":[]}')
    http = FakeHttp.new(response)
    relation = EmptyRelation.new
    controller = TeamController.new
    redirected = false
    Game.stub(:joins, relation) do
      Team.stub(:all, relation) do
        Net::HTTP.stub(:new, http) do
          ActiveRecord::Base.stub(:connection, -> { flunk "service-persisted series must not be inserted again" }) do
            controller.stub(:root_path, "/") do
              controller.stub(:redirect_back, ->(**options) { redirected = options[:fallback_location] == "/" }) do
                controller.historic_ratings
              end
            end
          end
        end
      end
    end
    assert redirected
    assert_equal "/historic_ratings", http.request_sent.path
    assert_equal 300, http.read_timeout
  end

  def test_http_failure_is_raised_before_response_is_decoded
    response = Net::HTTPInternalServerError.new("1.1", "500", "Internal Server Error")
    http = FakeHttp.new(response)
    relation = EmptyRelation.new
    Game.stub(:joins, relation) do
      Team.stub(:all, relation) do
        Net::HTTP.stub(:new, http) do
          ActiveSupport::JSON.stub(:decode, ->(*) { flunk "failed response must not be decoded" }) do
            assert_raises(Net::HTTPFatalError) { TeamController.new.historic_ratings }
          end
        end
      end
    end
  end

  def test_nonempty_legacy_series_is_still_persisted
    response = Struct.new(:body) do
      def value = self
    end.new('{"ratings":{"1":[50]},"offense":{"1":[1.2]},"defense":{"1":[1.3]},"dates":["2026-09-30"]}')
    http = FakeHttp.new(response)
    relation = EmptyRelation.new
    sql = []
    connection = Object.new
    connection.define_singleton_method(:execute) { |statement| sql << statement }
    controller = TeamController.new
    Game.stub(:joins, relation) do
      Team.stub(:all, relation) do
        Net::HTTP.stub(:new, http) do
          ActiveRecord::Base.stub(:connection, connection) do
            controller.stub(:root_path, "/") do
              controller.stub(:redirect_back, ->(**) {}) { controller.historic_ratings }
            end
          end
        end
      end
    end
    assert_equal 1, sql.length
    assert_includes sql.first, "(1, 1.2, 1.3, 50, '2026-09-30')"
    assert_includes sql.first, "ON DUPLICATE KEY UPDATE"
  end
end
