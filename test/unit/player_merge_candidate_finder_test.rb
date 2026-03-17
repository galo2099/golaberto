require File.dirname(__FILE__) + '/../test_helper'
require Rails.root.join('lib/player_merge_candidate_finder')

class PlayerMergeCandidateFinderTest < Test::Unit::TestCase
  FakePlayer = Struct.new(:name, :full_name)

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

    left = FakePlayer.new('Ana', 'Ana Maria')
    right = FakePlayer.new('Ana Clara', 'Ana Maria Souza')

    assert_in_delta 4.9, finder.similarity_score(left, right), 0.001
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

    left = FakePlayer.new('Ana', nil)
    right = FakePlayer.new('Ana Clara', nil)

    assert_in_delta 1.1, finder.similarity_score(left, right), 0.001
  end

  def test_suggest_threshold_prioritizes_full_name_match_coverage
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [
      { left: FakePlayer.new('A', 'Ana Maria'), right: FakePlayer.new('A2', 'Ana Maria'), score: 10.0 },
      { left: FakePlayer.new('B', 'Bruna Silva'), right: FakePlayer.new('B2', 'Bruna Silva'), score: 8.0 },
      { left: FakePlayer.new('C', 'Carla Souza'), right: FakePlayer.new('C2', 'Carla Souza'), score: 6.0 },
      { left: FakePlayer.new('X', 'X1'), right: FakePlayer.new('Y', 'Y1'), score: 20.0 },
    ]

    assert_in_delta 6.0, finder.suggest_threshold(candidates), 0.001
  end

  def test_suggest_threshold_falls_back_to_gap_when_no_full_name_matches
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [3.9, 3.6, 3.55, 2.1, 2.05].map do |score|
      { left: FakePlayer.new('A', nil), right: FakePlayer.new('B', nil), score: score }
    end

    assert_in_delta 2.1, finder.suggest_threshold(candidates), 0.001
  end
end
