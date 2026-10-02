class OddsRequestBuilder
  GAME_FIELDS = [:id, :home_id, :away_id, :home_score, :away_score, :played].freeze
  Rating = Struct.new(:measure_date, :off_rating, :def_rating)

  def initialize(group)
    @group = group
  end

  def build(games_json: nil, include_game_dates: false)
    @group.as_json(
      include: {
        phase: { include: :championship },
        team_groups: { only: [:team_id, :add_sub, :bias] }
      }
    ).merge('games' => games_json || games_json(include_dates: include_game_dates))
  end

  def games_json(include_dates: false)
    fields = include_dates ? GAME_FIELDS + [:date] : GAME_FIELDS
    # Fetch persisted fixtures even if this Group's association was loaded
    # earlier. The old includes-based request path also fetched a fresh relation.
    games = @group.games.select('games.*').to_a
    ratings = ratings_for(games)
    games.map do |game|
      boundary = rating_boundary(game.date)
      home_rating = latest_rating(ratings[game.home_id], boundary)
      away_rating = latest_rating(ratings[game.away_id], boundary)
      game.as_json(only: fields).merge(
        'home_power' => game.home_power(home_rating, away_rating),
        'away_power' => game.away_power(home_rating, away_rating)
      )
    end
  end

  private

  # Ratings are DATEs, but games can have a time of day. Strict-before includes
  # today's rating after UTC midnight, and excludes it at exactly UTC midnight. Compare
  # DATEs in SQL so the existing (team_id, measure_date) index remains usable.
  def rating_boundary(time)
    return nil if time.nil?

    time = time.getutc
    date = time.to_date
    time == time.beginning_of_day ? date : date + 1
  end

  def ratings_for(games)
    boundaries = games.filter_map { |game| rating_boundary(game.date) }
    return {} if boundaries.empty?

    first, last = boundaries.minmax
    team_ids = games.flat_map { |game| [game.home_id, game.away_id] }.compact.uniq
    scope = HistoricalRating.where(team_id: team_ids)
    previous_dates = scope.where('measure_date < ?', first).group(:team_id).maximum(:measure_date)
    selected = scope.where(measure_date: first...last)
    previous_dates.each do |team_id, date|
      selected = selected.or(HistoricalRating.where(team_id: team_id, measure_date: date))
    end
    ratings = Hash.new { |hash, key| hash[key] = [] }
    selected.order(:measure_date).pluck(:team_id, :measure_date, :off_rating, :def_rating).each do |team_id, date, off, defense|
      ratings[team_id] << Rating.new(date, off, defense)
    end
    ratings
  end

  def latest_rating(ratings, boundary)
    return nil if boundary.nil? || ratings.nil? || ratings.empty?

    index = ratings.bsearch_index { |rating| rating.measure_date >= boundary }
    return ratings.last if index.nil?
    return nil if index.zero?

    ratings[index - 1]
  end
end
