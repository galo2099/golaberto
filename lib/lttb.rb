module LTTB
  def self.downsample(data, threshold)
    return data if threshold >= data.length || threshold <= 2

    sampled = []
    sampled << data[0]

    # Bucket size. Leave room for start and end data points
    every = (data.length - 2).to_f / (threshold - 2)

    a = 0 # Initially a is the first point in the triangle

    (0...(threshold - 2)).each do |i|
      # Calculate point average for next bucket (containing c)
      avg_x = 0
      avg_y = 0
      avg_range_start = ((i + 1) * every).floor + 1
      avg_range_end = ((i + 2) * every).floor + 1
      avg_range_end = [avg_range_end, data.length].min

      avg_range_length = avg_range_end - avg_range_start

      (avg_range_start...avg_range_end).each do |j|
        avg_x += data[j][0].to_f
        avg_y += data[j][1].to_f
      end

      avg_x /= avg_range_length
      avg_y /= avg_range_length

      # Get the range for this bucket
      range_offs = ((i + 0) * every).floor + 1
      range_to = ((i + 1) * every).floor + 1

      # Point a
      point_a_x = data[a][0].to_f
      point_a_y = data[a][1].to_f

      max_area = -1
      next_a = range_offs

      (range_offs...range_to).each do |j|
        # Calculate triangle area over three buckets
        area = ((point_a_x - avg_x) * (data[j][1].to_f - point_a_y) - (point_a_x - data[j][0].to_f) * (avg_y - point_a_y)).abs * 0.5
        if area > max_area
          max_area = area
          next_a = j
        end
      end

      sampled << data[next_a]
      a = next_a # This becomes the next point a
    end

    sampled << data.last

    sampled
  end
end
