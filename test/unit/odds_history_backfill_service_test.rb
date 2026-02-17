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
      FakeTeamGroups.new
    end

    def games
      @games
    end

    def odds(games_json:, **_args)
      @captured_games_json << games_json
    end
  end

  test "uses powers from first unplayed game and includes a pre-play snapshot" do
    base_games_json = [
      {
        "id" => 1,
        "home_id" => 10,
        "away_id" => 20,
        "home_score" => 1,
        "away_score" => 0,
        "played" => true,
        "date" => "2024-06-01",
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
        "home_power" => 3.3,
        "away_power" => 1.4,
      },
    ]

    fake_group = FakeGroup.new(base_games_json)
    fake_relation = FakeRelation.new(fake_group)

    Group.stub(:joins, fake_relation) do
      OddsHistoryBackfillService.run_unlocked(championship_id: 1)
    end

    pre_play_snapshot = fake_group.captured_games_json[0]
    june_first_snapshot = fake_group.captured_games_json[1]
    june_fifth_snapshot = fake_group.captured_games_json[2]
    june_tenth_snapshot = fake_group.captured_games_json[3]

    assert_equal false, pre_play_snapshot.find { |game| game["id"] == 1 }["played"]

    pre_play_unplayed_game = pre_play_snapshot.find { |game| game["id"] == 3 }
    june_first_unplayed_game = june_first_snapshot.find { |game| game["id"] == 3 }
    june_fifth_unplayed_game = june_fifth_snapshot.find { |game| game["id"] == 3 }
    june_tenth_played_game = june_tenth_snapshot.find { |game| game["id"] == 3 }

    assert_equal 1.1, pre_play_unplayed_game["home_power"]
    assert_equal 0.9, pre_play_unplayed_game["away_power"]

    assert_equal 2.2, june_first_unplayed_game["home_power"]
    assert_equal 1.1, june_first_unplayed_game["away_power"]

    assert_equal 3.3, june_fifth_unplayed_game["home_power"]
    assert_equal 1.4, june_fifth_unplayed_game["away_power"]

    assert_equal true, june_tenth_played_game["played"]
    assert_equal 3.3, june_tenth_played_game["home_power"]
    assert_equal 1.4, june_tenth_played_game["away_power"]
  end
end
