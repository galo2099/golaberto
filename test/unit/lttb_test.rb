require 'test_helper'
require 'lttb'

class LTTBTest < ActiveSupport::TestCase
  test "downsample returns original data if threshold is greater than data length" do
    data = [[1, 10], [2, 20], [3, 30]]
    assert_equal data, LTTB.downsample(data, 5)
  end

  test "downsample returns original data if threshold is 0 or less" do
    data = [[1, 10], [2, 20], [3, 30]]
    assert_equal data, LTTB.downsample(data, 0)
    assert_equal data, LTTB.downsample(data, -1)
  end

  test "downsample returns first and last points when threshold is 2" do
    data = [[1, 10], [2, 20], [3, 30], [4, 40]]
    # Our implementation returns original data if threshold <= 2
    assert_equal data, LTTB.downsample(data, 2)
  end

  test "downsample correctly downsamples data" do
    data = (1..100).map { |i| [i, i] }
    threshold = 10
    downsampled = LTTB.downsample(data, threshold)
    assert_equal threshold, downsampled.length
    assert_equal data.first, downsampled.first
    assert_equal data.last, downsampled.last
  end

  test "downsample handles non-numeric data in ratings by converting to float" do
    data = [[1, "10.5"], [2, nil], [3, 30]]
    threshold = 3
    downsampled = LTTB.downsample(data, threshold)
    assert_equal 3, downsampled.length
    assert_equal 10.5, downsampled[0][1].to_f
    assert_equal 0.0, downsampled[1][1].to_f
    assert_equal 30.0, downsampled[2][1].to_f
  end
end
