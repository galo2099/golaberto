require 'geokdtree'

module GeoClusterer
  RADIUS = 60
  EXTENT = 512

  # Earth's radius in km
  EARTH_RADIUS_KM = 6371

  class << self
  def haversine_distance(lat1, lon1, lat2, lon2)
    # Convert latitude and longitude from degrees to radians
    lat1_rad = lat1 * Math::PI / 180
    lon1_rad = lon1 * Math::PI / 180
    lat2_rad = lat2 * Math::PI / 180
    lon2_rad = lon2 * Math::PI / 180

    # Difference in coordinates
    dlat = lat2_rad - lat1_rad
    dlon = lon2_rad - lon1_rad

    # Haversine formula
    a = Math.sin(dlat / 2) ** 2 + Math.cos(lat1_rad) * Math.cos(lat2_rad) * Math.sin(dlon / 2) ** 2
    c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a))

    # Distance in km
    distance_km = EARTH_RADIUS_KM * c

    return distance_km
  end

  def get_bounds_zoom_level(bounds, map)
    max_zoom = 21
    min_zoom = 0

    ne = bounds[:northeast] # Assuming bounds are given as a hash with :northeast and :southwest keys
    sw = bounds[:southwest] # {northeast: {lat: x, lng: y}, southwest: {lat: x, lng: y}}

    world_coord_width = (ne[:lng] - sw[:lng]).abs
    world_coord_height = (ne[:lat] - sw[:lat]).abs

    fit_pad = 40

    max_zoom.downto(min_zoom) do |zoom|
      if world_coord_width * (1 << zoom) + 2 * fit_pad < map[:width] &&
         world_coord_height * (1 << zoom) + 2 * fit_pad < map[:height]
        return zoom
      end
    end

    0
  end

  def cluster(points, z)
    # Prepare points
    data = []

    points.each_with_index do |p, i|
      next unless p[:geometry] # Skip if no geometry

      lng, lat = p[:geometry][:coordinates]
      x = lng_x(lng)
      y = lat_y(lat)

      # Store point/cluster data
      cluster_data = [x, y, i]
      data.push(cluster_data)
    end

    tree = create_tree(data)

    _cluster(data, tree, z)
  end

 private
  def create_tree(points)
    tree = Geokdtree::Tree.new(2)
    points.each_with_index do |p, i|
      tree.insert([p[0], p[1]], i)
    end
    tree
  end

  def _cluster(points, tree, zoom)
    r = RADIUS.to_f / (EXTENT * (2 ** zoom))
    visited = {}
    clusters = []

    points.each_with_index do |point_data, i|
      next if visited[i]

      visited[i] = true
      x, y = point_data[0], point_data[1]
      neighbors = tree.nearest_range([x, y], r)

      if neighbors.size > 0
        wx = x
        wy = y
        num_points = 1

        neighbors.each do |neighbor|
          next if visited[neighbor.data]

          visited[neighbor.data] = true
          wx += neighbor.point[0]
          wy += neighbor.point[1]
          num_points += 1
        end

        clusters << [wx / num_points, wy / num_points, num_points]
      else
        clusters << [x, y, 1]
      end
    end
    clusters.map{|c| [x_lng(c[0]), y_lat(c[1]), c[2]] }
  end

  def lng_x(lng)
    lng / 360.0 + 0.5
  end

  def lat_y(lat)
    sin = Math.sin(lat * Math::PI / 180)
    y = 0.5 - 0.25 * Math.log((1 + sin) / (1 - sin)) / Math::PI
    [[y, 0].max, 1].min
  end

  def x_lng(x)
    (x - 0.5) * 360
  end

  def y_lat(y)
    y2 = (180 - y * 360) * Math::PI / 180
    360 * Math.atan(Math.exp(y2)) / Math::PI - 90
  end
  end
end
