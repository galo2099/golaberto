require "test_helper"

class OddsHistoryBackfillServiceTest < ActiveSupport::TestCase
  class FakeRelation
    def initialize(group)
      @group = group
    end

    def where(*_args)
      self
    end

    def includes(*_args)
      self
    end

    def size
      1
    end

    def find_each
      yield @group
    end
  end

  class FakeGames
    def initialize(base_games_json)
      @base_games_json = base_games_json
    end

    def includes(*_args)
      self
    end

    def as_json(*_args)
      @base_games_json
    end
  end

  class FakeTeamGroups
    def initialize(team_ids)
      @team_ids = team_ids
    end

    def map
      return enum_for(:map) unless block_given?

      @team_ids.map { |team_id| yield Struct.new(:team_id).new(team_id) }
    end

    def find_each
    end
  end

  class FakeGroup
    attr_reader :captured_games_json

    def initialize(base_games_json)
      @games = FakeGames.new(base_games_json)
      @captured_games_json = []
    end

    def id
      99
    end

    def name
      "Test Group"
    end

    def team_groups
      ids = @games.as_json.flat_map { |game| [ game["home_id"], game["away_id"] ] }.compact.uniq
      FakeTeamGroups.new(ids)
    end

    def games
      @games
    end

    def odds(games_json:, **_args)
      @captured_games_json << games_json
    end
  end

  class FakeHistoricalScope
    def initialize(ratings)
      @ratings = ratings
    end

    def order(*_args)
      @ratings.sort_by(&:measure_date)
    end
  end

  test "uses ratings from day after last played game when backfilling each snapshot" do
    base_games_json = [
      {
        "id" => 1,
        "home_id" => 10,
        "away_id" => 20,
        "home_score" => 1,
        "away_score" => 0,
        "played" => true,
        "date" => "2024-06-01",
        "home_field" => "left",
        "home_power" => 1.1,
        "away_power" => 0.9,
      },
      {
        "id" => 2,
        "home_id" => 30,
        "away_id" => 40,
        "home_score" => 2,
        "away_score" => 1,
        "played" => true,
        "date" => "2024-06-05",
        "home_field" => "left",
        "home_power" => 2.2,
        "away_power" => 1.1,
      },
      {
        "id" => 3,
        "home_id" => 50,
        "away_id" => 60,
        "home_score" => 0,
        "away_score" => 0,
        "played" => true,
        "date" => "2024-06-10",
        "home_field" => "left",
        "home_power" => 3.3,
        "away_power" => 1.4,
      },
    ]

    rating = Struct.new(:team_id, :measure_date, :off_rating, :def_rating)
    all_ratings = [
      rating.new(50, Date.parse("2024-05-30"), 1.2, 1.0),
      rating.new(60, Date.parse("2024-05-30"), 1.1, 0.9),
      rating.new(50, Date.parse("2024-06-01"), 1.8, 1.2),
      rating.new(60, Date.parse("2024-06-01"), 1.6, 1.4),
      rating.new(50, Date.parse("2024-06-05"), 2.6, 1.8),
      rating.new(60, Date.parse("2024-06-05"), 2.1, 1.9),
    ]

    fake_group = FakeGroup.new(base_games_json)
    fake_relation = FakeRelation.new(fake_group)

    Group.stub(:joins, fake_relation) do
      HistoricalRating.stub(:where, FakeHistoricalScope.new(all_ratings)) do
        OddsHistoryBackfillService.run_unlocked(championship_id: 1)
      end
    end

    pre_play_snapshot = fake_group.captured_games_json[0].find { |game| game["id"] == 3 }
    june_first_snapshot = fake_group.captured_games_json[1].find { |game| game["id"] == 3 }
    june_fifth_snapshot = fake_group.captured_games_json[2].find { |game| game["id"] == 3 }

    game_for_power = Game.new(home_field: "left")

    expected_pre = [
      game_for_power.home_power(all_ratings[0], all_ratings[1]),
      game_for_power.away_power(all_ratings[0], all_ratings[1]),
    ]
    expected_june_first = [
      game_for_power.home_power(all_ratings[2], all_ratings[3]),
      game_for_power.away_power(all_ratings[2], all_ratings[3]),
    ]
    expected_june_fifth = [
      game_for_power.home_power(all_ratings[4], all_ratings[5]),
      game_for_power.away_power(all_ratings[4], all_ratings[5]),
    ]

    assert_in_delta expected_pre[0], pre_play_snapshot["home_power"], 0.000001
    assert_in_delta expected_pre[1], pre_play_snapshot["away_power"], 0.000001

    assert_in_delta expected_june_first[0], june_first_snapshot["home_power"], 0.000001
    assert_in_delta expected_june_first[1], june_first_snapshot["away_power"], 0.000001

    assert_in_delta expected_june_fifth[0], june_fifth_snapshot["home_power"], 0.000001
    assert_in_delta expected_june_fifth[1], june_fifth_snapshot["away_power"], 0.000001
  end
end
