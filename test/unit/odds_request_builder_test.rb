require 'test_helper'

class OddsRequestBuilderTest < ActiveSupport::TestCase
  setup do
    category = Category.create!(name: 'Request builder category')
    championship = Championship.create!(
      name: 'Request builder championship', region: :national, category: category,
      begin: Date.new(2024, 1, 1), end: Date.new(2024, 12, 31),
      point_win: 3, point_draw: 1, point_loss: 0
    )
    @phase = Phase.create!(name: 'Request phase', championship: championship, order_by: 1, sort: 'pt,w,gd,gf')
    @group = Group.create!(name: 'Request group', phase: @phase)
    Net::HTTP.stub(:get, '[]') do
      @home = Team.create!(name: 'Request home', country: 'Brazil')
      @away = Team.create!(name: 'Request away', country: 'Brazil')
    end
    [@home, @away].each { |team| TeamGroup.create!(group: @group, team: team, add_sub: 0, bias: 0) }
    [@home, @away].each_with_index do |team, i|
      [[Date.new(2024, 1, 1), 1.7 + i], [Date.new(2024, 1, 10), 3.2 + i], [Date.new(2024, 2, 1), 8.0 + i]].each do |date, off|
        HistoricalRating.create!(team: team, measure_date: date, off_rating: off, def_rating: 0.6 + i, rating: 1.0)
      end
    end
  end

  test 'matches complete legacy JSON including strict dates and every home field' do
    ['left', 'neutral', 'right'].each do |field|
      add_game(Time.utc(2024, 1, 10), field)
      add_game(Time.utc(2024, 1, 10, 0, 0, 1), field, played: true)
      add_game(Time.utc(2024, 1, 11), field)
    end
    add_game(Time.utc(2023, 12, 31), 'left') # No earlier rating.
    assert_equal legacy_request.to_json, @group.odds_request.to_json
    games = OddsRequestBuilder.new(@group).games_json
    assert_nil games.first.fetch('home_power')
    assert_nil games.first.fetch('away_power')
    assert_not_equal games[1]['home_power'], games[2]['home_power']
  end

  test 'rating selection always uses UTC across user timezones' do
    midnight = add_game(Time.utc(2024, 1, 10), 'right')
    add_game(Time.utc(2024, 1, 10, 0, 0, 1), 'right')
    expected = @group.odds_request['games']
    ['UTC', 'America/Sao_Paulo', 'America/Los_Angeles', 'Asia/Tokyo'].each do |zone|
      Time.use_zone(zone) do
        request = Group.find(@group.id).odds_request
        assert_equal expected, request['games']
        request['games'].each do |game_json|
          game = Game.find(game_json['id'])
          rating_date = game.id == midnight.id ? Date.new(2024, 1, 1) : Date.new(2024, 1, 10)
          home_rating = HistoricalRating.find_by!(team: @home, measure_date: rating_date)
          away_rating = HistoricalRating.find_by!(team: @away, measure_date: rating_date)
          assert_equal game.home_power(home_rating, away_rating), game_json['home_power']
          assert_equal game.away_power(home_rating, away_rating), game_json['away_power']
        end
      end
    end
  end

  test 'UTC rating boundary does not depend on database timezone settings' do
    builder = OddsRequestBuilder.new(@group)
    ActiveRecord.stub(:default_timezone, :local) do
      # Both timestamps are on Jan 9 in this offset, but on Jan 10 in UTC.
      assert_equal Date.new(2024, 1, 10), builder.send(:rating_boundary, Time.utc(2024, 1, 10).getlocal('-03:00'))
      assert_equal Date.new(2024, 1, 11), builder.send(:rating_boundary, Time.utc(2024, 1, 10, 1).getlocal('-03:00'))
    end
  end

  test 'includes external opponents and preserves missing rating values' do
    outsider = nil
    Net::HTTP.stub(:get, '[]') { outsider = Team.create!(name: 'Request outsider', country: 'Brazil') }
    add_game(Time.utc(2024, 1, 11), 'neutral', away: outsider)
    assert_equal legacy_request.to_json, @group.odds_request.to_json
    assert_nil @group.odds_request['games'].first['home_power']
  end

  test 'supplied snapshot games bypass all historical rating queries' do
    add_game(Time.utc(2024, 1, 11), 'left')
    queries = rating_queries do
      supplied = [{'id' => 12, 'home_power' => 0.2, 'played' => false}]
      assert_same supplied, @group.odds_request(games_json: supplied)['games']
      assert_equal [], @group.odds_request(games_json: [])['games']
    end
    assert_empty queries
  end

  test 'refreshes persisted fixtures when the association was already loaded' do
    game = add_game(Time.utc(2024, 1, 11), 'neutral')
    @group.games.load
    game.update_columns(home_score: 2)
    assert_equal 2, @group.odds_request['games'].first['home_score']
    assert_equal legacy_request.to_json, @group.odds_request.to_json
  end

  test 'rating query count stays bounded as fixtures increase and optional dates match' do
    20.times { |i| add_game(Time.utc(2024, 1, 2) + i.days, 'neutral') }
    queries = rating_queries { @group.odds_request }
    assert_equal 2, queries.size
    expected = @group.games.includes(:home, :away).as_json(
      methods: [:home_power, :away_power], only: OddsRequestBuilder::GAME_FIELDS + [:date]
    )
    assert_equal expected, OddsRequestBuilder.new(@group).games_json(include_dates: true)
  end

  private

  def add_game(date, field, played: false, away: @away)
    Game.create!(phase: @phase, home: @home, away: away, date: date,
      home_score: 0, away_score: 0, played: played, home_field: field)
  end

  def legacy_request(group = @group)
    games = group.games.includes(:home, :away).as_json(
      methods: [:home_power, :away_power], only: OddsRequestBuilder::GAME_FIELDS
    )
    group.as_json(include: {
      phase: {include: :championship}, team_groups: {only: [:team_id, :add_sub, :bias]}
    }).merge('games' => games)
  end

  def rating_queries
    queries = []
    callback = ->(*args) do
      payload = args.last
      queries << payload[:sql] if !payload[:cached] && payload[:sql].include?('historical_ratings')
    end
    ActiveRecord::Base.uncached do
      ActiveSupport::Notifications.subscribed(callback, 'sql.active_record') { yield }
    end
    queries
  end
end
