require 'test_helper'

class GameOddsRatingTest < ActiveSupport::TestCase
  setup do
    category = Category.create!(name: 'Game odds rating category')
    championship = Championship.create!(
      name: 'Game odds rating championship', region: :national, category: category,
      begin: Date.new(2024, 1, 1), end: Date.new(2024, 12, 31),
      point_win: 3, point_draw: 1, point_loss: 0
    )
    @phase = Phase.create!(name: 'Game odds rating phase', championship: championship,
      order_by: 1, sort: 'pt,w,gd,gf')
    @group = Group.create!(name: 'Game odds rating group', phase: @phase)
    Net::HTTP.stub(:get, '[]') do
      @home = Team.create!(name: 'Game odds rating home', country: 'Brazil')
      @away = Team.create!(name: 'Game odds rating away', country: 'Brazil')
    end
    [@home, @away].each_with_index do |team, index|
      [Date.new(2024, 1, 1), Date.new(2024, 1, 10), Date.new(2024, 1, 11)].each_with_index do |date, day|
        HistoricalRating.create!(
          team: team, measure_date: date, off_rating: 1.5 + index + day,
          def_rating: 0.5 + index + day, rating: 1.0
        )
      end
    end
  end

  test 'played games exclude the whole UTC matchday while scheduled games use strict-before time' do
    scheduled = add_game(Time.utc(2024, 1, 10, 12), played: false)
    played = add_game(Time.utc(2024, 1, 10, 12), played: true)

    assert_equal Date.new(2024, 1, 11), scheduled.historical_rating_boundary
    assert_equal Date.new(2024, 1, 10), played.historical_rating_boundary
    assert_equal Date.new(2024, 1, 10), scheduled.home_rating.measure_date
    assert_equal Date.new(2024, 1, 1), played.home_rating.measure_date
    assert_equal Date.new(2024, 1, 10), scheduled.away_rating.measure_date
    assert_equal Date.new(2024, 1, 1), played.away_rating.measure_date
  end

  test 'UTC offsets determine both calendar day and strict-before midnight boundary' do
    utc_midnight = Time.new(2024, 1, 9, 21, 0, 0, '-03:00')
    after_utc_midnight = Time.new(2024, 1, 10, 0, 0, 1, '-03:00')
    at_midnight = add_game(utc_midnight, played: false)
    later_scheduled = add_game(after_utc_midnight, played: false)
    later_played = add_game(after_utc_midnight, played: true)

    assert_equal Date.new(2024, 1, 10), at_midnight.historical_rating_boundary
    assert_equal Date.new(2024, 1, 11), later_scheduled.historical_rating_boundary
    assert_equal Date.new(2024, 1, 10), later_played.historical_rating_boundary
    assert_equal Date.new(2024, 1, 1), at_midnight.home_rating.measure_date
    assert_equal Date.new(2024, 1, 10), later_scheduled.home_rating.measure_date
    assert_equal Date.new(2024, 1, 1), later_played.home_rating.measure_date
  end

  test 'changing played after loading ratings invalidates both sides before and after save' do
    game = add_game(Time.utc(2024, 1, 10, 12), played: false)
    assert_equal Date.new(2024, 1, 10), game.home_rating.measure_date
    assert_equal Date.new(2024, 1, 10), game.away_rating.measure_date

    game.played = true
    assert_equal Date.new(2024, 1, 1), game.home_rating.measure_date
    assert_equal Date.new(2024, 1, 1), game.away_rating.measure_date
    game.save!
    assert_equal Date.new(2024, 1, 1), game.home_rating.measure_date
    assert_equal Date.new(2024, 1, 1), game.away_rating.measure_date
  end

  test 'unsaved date changes invalidate an owner-dependent association scope' do
    preloaded = add_game(Time.utc(2024, 1, 10, 12), played: false)
    assert_equal Date.new(2024, 1, 10), preloaded.association(:home_rating).reader.measure_date
    preloaded.date = Time.utc(2024, 1, 11, 12)
    assert_equal Date.new(2024, 1, 11), preloaded.home_rating.measure_date

    previously_read = add_game(Time.utc(2024, 1, 10, 12), played: false)
    assert_equal Date.new(2024, 1, 10), previously_read.home_rating.measure_date
    previously_read.date = Time.utc(2024, 1, 11, 12)
    assert_equal Date.new(2024, 1, 11), previously_read.home_rating.measure_date
  end

  test 'versioned games share the played-match rating cutoff' do
    game = add_game(Time.utc(2024, 1, 10, 12), played: true)
    version = game.versions.last

    assert_equal Date.new(2024, 1, 10), version.historical_rating_boundary
    assert_equal Date.new(2024, 1, 1), version.home_rating.measure_date
    assert_equal Date.new(2024, 1, 1), version.away_rating.measure_date
  end

  test 'same-day rating updates after kickoff do not change played odds or batched powers' do
    game = add_game(Time.utc(2024, 1, 10, 12), played: true)
    game.update!(home_score: 5, away_score: 0)
    original_home = game.home_rating
    original_away = game.away_rating
    original_powers = [game.home_power, game.away_power]
    original_odds = game.odds
    assert_equal Date.new(2024, 1, 1), original_home.measure_date
    assert_equal Date.new(2024, 1, 1), original_away.measure_date

    HistoricalRating.find_by!(team: @home, measure_date: Date.new(2024, 1, 10))
      .update!(off_rating: 20.0, def_rating: 20.0)
    HistoricalRating.find_by!(team: @away, measure_date: Date.new(2024, 1, 10))
      .update!(off_rating: 30.0, def_rating: 30.0)

    game.reload
    assert_equal original_powers, [game.home_power, game.away_power]
    assert_equal original_odds, game.odds
    assert_equal original_powers, batched_powers(game.id)
  end

  test 'no earlier rating remains nil for played odds without same-day fallback' do
    team = nil
    Net::HTTP.stub(:get, '[]') do
      team = Team.create!(name: 'Game odds rating no prior', country: 'Brazil')
    end
    HistoricalRating.create!(team: team, measure_date: Date.new(2024, 1, 10),
      off_rating: 2.0, def_rating: 0.7, rating: 1.0)
    TeamGroup.create!(group: @group, team: team, add_sub: 0, bias: 0)
    played = Game.create!(phase: @phase, home: team, away: @away,
      date: Time.utc(2024, 1, 10, 12), home_score: 1, away_score: 0,
      played: true, home_field: 'left')

    assert_nil played.home_rating
    assert_nil played.home_power
    assert_nil played.away_power
    assert_nil played.odds
    played_json = OddsRequestBuilder.new(@group).games_json.find { |item| item['id'] == played.id }
    assert_nil played_json['home_power']
    assert_nil played_json['away_power']

    scheduled = Game.create!(phase: @phase, home: team, away: @away,
      date: Time.utc(2024, 1, 10, 0, 0, 1), home_score: 0, away_score: 0,
      played: false, home_field: 'left')
    assert_equal Date.new(2024, 1, 10), scheduled.home_rating.measure_date
    scheduled_json = OddsRequestBuilder.new(@group).games_json.find { |item| item['id'] == scheduled.id }
    assert_not_nil scheduled_json['home_power']
    assert_not_nil scheduled_json['away_power']
  end

  private

  def add_game(date, played:)
    game = Game.create!(phase: @phase, home: @home, away: @away,
      date: date, home_score: 0, away_score: 0, played: played,
      home_field: 'left')
    TeamGroup.create!(group: @group, team: @home, add_sub: 0, bias: 0) unless @group.team_groups.exists?(team: @home)
    TeamGroup.create!(group: @group, team: @away, add_sub: 0, bias: 0) unless @group.team_groups.exists?(team: @away)
    game
  end

  def batched_powers(game_id)
    json = OddsRequestBuilder.new(@group).games_json.find { |item| item['id'] == game_id }
    [json['home_power'], json['away_power']]
  end
end
