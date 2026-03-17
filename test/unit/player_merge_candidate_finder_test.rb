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

  def test_similarity_score_adds_name_and_full_name_scores
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

    assert_in_delta 3.0, finder.similarity_score(left, right), 0.001
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

  def test_suggest_threshold_uses_biggest_gap
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [3.9, 3.6, 3.55, 2.1, 2.05].map { |score| { score: score } }

    assert_in_delta 2.1, finder.suggest_threshold(candidates), 0.001
  end
end
