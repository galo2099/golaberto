# Read-only: build requests without posting /odds or persisting any records.
# bin/rails runner script/benchmark_odds_request.rb [ITERATIONS] [GROUP_ID...]
require 'digest'

iterations = Integer(ARGV.shift || 3)
ids = ARGV.empty? ? [16498, 16653, 16982] : ARGV.map { |id| Integer(id) }
abort 'ITERATIONS must be positive' unless iterations.positive?
ActiveRecord::Base.logger = nil

def legacy_odds_request(group)
  games_json = group.games.includes(:home, :away).as_json(
    methods: [:home_power, :away_power], only: OddsRequestBuilder::GAME_FIELDS
  )
  group.as_json(include: {
    phase: { include: :championship },
    team_groups: { only: [:team_id, :add_sub, :bias] }
  }).merge(games: games_json)
end

def measure_request(id, mode)
  sql = []
  subscriber = ActiveSupport::Notifications.subscribe('sql.active_record') do |_, start, finish, _, payload|
    if !payload[:cached] && payload[:name] != 'SCHEMA' && payload[:sql].lstrip.start_with?('SELECT')
      sql << {ms: 1000 * (finish - start), rating: payload[:sql].include?('historical_ratings')}
    end
  end
  before = GC.stat(:total_allocated_objects)
  started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  group = Group.find(id)
  request = mode == :legacy ? legacy_odds_request(group) : group.odds_request
  body = request.to_json
  ms = 1000 * (Process.clock_gettime(Process::CLOCK_MONOTONIC) - started)
  [body, {
    group: id, mode: mode, ms: ms, sql_ms: sql.sum { |q| q[:ms] },
    queries: sql.size, rating_queries: sql.count { |q| q[:rating] },
    games: (request['games'] || request[:games]).size, bytes: body.bytesize,
    allocations: GC.stat(:total_allocated_objects) - before,
    sha256: Digest::SHA256.hexdigest(body)
  }]
ensure
  ActiveSupport::Notifications.unsubscribe(subscriber) if subscriber
end

# Warm schemas/autoloading, never cache the measured SELECTs.
ActiveRecord::Base.uncached { legacy_odds_request(Group.find(ids.first)); Group.find(ids.first).odds_request }
ids.each do |id|
  iterations.times do |run|
    ActiveRecord::Base.connection.execute('SET TRANSACTION READ ONLY')
    ActiveRecord::Base.transaction(isolation: :repeatable_read) do
      ActiveRecord::Base.uncached do
        # Both builders see the same snapshot even if a scraper updates the DB.
        results = {}
        order = run.even? ? [:legacy, :bulk] : [:bulk, :legacy]
        order.each { |mode| results[mode] = measure_request(id, mode) }
        abort "Request bytes changed for group #{id}" unless results[:legacy][0] == results[:bulk][0]
        order.each { |mode| puts results[mode][1].merge(run: run, identical: true).to_json }
      end
    end
  end
end
