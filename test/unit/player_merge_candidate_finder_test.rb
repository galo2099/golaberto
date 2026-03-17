require File.dirname(__FILE__) + '/../test_helper'
require Rails.root.join('lib/player_merge_candidate_finder')

class PlayerMergeCandidateFinderTest < Test::Unit::TestCase
  FakePlayer = Struct.new(:name, :full_name, :sofascore_id, :soccerway_id, keyword_init: true)

  def test_find_candidates_ignores_players_with_nil_birthdate
    Player.create!(name: 'Ana One', country: 'Brazil', birth: nil)
    Player.create!(name: 'Ana Two', country: 'Brazil', birth: nil)

    finder = PlayerMergeCandidateFinder.new

    assert_equal [], finder.find_candidates
  end

  def test_similarity_score_gives_higher_weight_to_full_name
    fuzzy = Struct.new(:scores) do
      def getDistance(left, right)
        scores.fetch([left, right], 0)
      end
    end.new({
      ['Ana', 'Ana Clara'] => 1.1,
      ['Ana Maria', 'Ana Maria Souza'] => 1.9,
    })

    finder = PlayerMergeCandidateFinder.new(Player.none, fuzzy)

    left = FakePlayer.new(name: 'Ana', full_name: 'Ana Maria')
    right = FakePlayer.new(name: 'Ana Clara', full_name: 'Ana Maria Souza')

    assert_in_delta 4.9, finder.similarity_score(left, right), 0.001
  end

  def test_similarity_score_adds_bonus_for_complementary_external_ids
    finder = PlayerMergeCandidateFinder.new(Player.none, Struct.new(:dummy) do
      def getDistance(_left, _right)
        1.0
      end
    end.new(nil))

    left = FakePlayer.new(name: 'Ana', full_name: 'Ana Maria', sofascore_id: 'sofa-1', soccerway_id: nil)
    right = FakePlayer.new(name: 'Ana', full_name: 'Ana Maria', sofascore_id: nil, soccerway_id: 'sw-1')

    assert_in_delta 8.0, finder.similarity_score(left, right), 0.001
  end

  def test_similarity_score_handles_blank_full_name
    fuzzy = Struct.new(:scores) do
      def getDistance(left, right)
        scores.fetch([left, right], 0)
      end
    end.new({
      ['Ana', 'Ana Clara'] => 1.1,
    })

    finder = PlayerMergeCandidateFinder.new(Player.none, fuzzy)

    left = FakePlayer.new(name: 'Ana', full_name: nil)
    right = FakePlayer.new(name: 'Ana Clara', full_name: nil)

    assert_in_delta 1.1, finder.similarity_score(left, right), 0.001
  end

  def test_suggest_threshold_prioritizes_full_name_match_coverage
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [
      { left: FakePlayer.new(name: 'A', full_name: 'Ana Maria'), right: FakePlayer.new(name: 'A2', full_name: 'Ana Maria'), score: 10.0 },
      { left: FakePlayer.new(name: 'B', full_name: 'Bruna Silva'), right: FakePlayer.new(name: 'B2', full_name: 'Bruna Silva'), score: 8.0 },
      { left: FakePlayer.new(name: 'C', full_name: 'Carla Souza'), right: FakePlayer.new(name: 'C2', full_name: 'Carla Souza'), score: 6.0 },
      { left: FakePlayer.new(name: 'X', full_name: 'X1'), right: FakePlayer.new(name: 'Y', full_name: 'Y1'), score: 20.0 },
    ]

    assert_in_delta 6.0, finder.suggest_threshold(candidates), 0.001
  end

  def test_suggest_threshold_falls_back_to_gap_when_no_full_name_matches
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [3.9, 3.6, 3.55, 2.1, 2.05].map do |score|
      { left: FakePlayer.new(name: 'A', full_name: nil), right: FakePlayer.new(name: 'B', full_name: nil), score: score }
    end

    assert_in_delta 2.1, finder.suggest_threshold(candidates), 0.001
  end
end
