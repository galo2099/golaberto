require File.dirname(__FILE__) + '/../test_helper'

class TeamTest < Test::Unit::TestCase
  def test_get_historical_ratings_returns_full_data_without_threshold
    plucked_rows = [[Time.utc(2024, 1, 1), "12.5"], [Time.utc(2024, 1, 2), 13]]
    historical_scope = Object.new
    historical_scope.define_singleton_method(:order) do |field|
      raise "unexpected order field" unless field == :measure_date
      self
    end
    historical_scope.define_singleton_method(:pluck) do |*fields|
      raise "unexpected pluck fields" unless fields == [:measure_date, :rating]
      plucked_rows
    end

    HistoricalRating.stub(:where, historical_scope) do
      assert_equal [[1704067200, 12.5], [1704153600, 13.0]], Team.get_historical_ratings(10)
    end
  end

  def test_get_historical_ratings_downsamples_when_threshold_is_provided
    data_rows = [[Time.utc(2024, 1, 1), 10.0], [Time.utc(2024, 1, 2), 11.0], [Time.utc(2024, 1, 3), 12.0]]
    expected_data = data_rows.map { |date, rating| [date.to_i, rating.to_f] }
    downsample_calls = []
    historical_scope = Object.new
    historical_scope.define_singleton_method(:order) do |field|
      raise "unexpected order field" unless field == :measure_date
      self
    end
    historical_scope.define_singleton_method(:pluck) do |*fields|
      raise "unexpected pluck fields" unless fields == [:measure_date, :rating]
      data_rows
    end

    HistoricalRating.stub(:where, historical_scope) do
      LTTB.stub(:downsample, lambda { |data, threshold|
        downsample_calls << [data, threshold]
        [["downsampled"]]
      }) do
        assert_equal [["downsampled"]], Team.get_historical_ratings(10, 2)
      end
    end

    assert_equal [[expected_data, 2]], downsample_calls
  end

  # Replace this with your real tests.
  def test_truth
    assert true
  end
end
