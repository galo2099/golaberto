require File.dirname(__FILE__) + '/../test_helper'
require Rails.root.join('lib/player_merge_candidate_finder')

class PlayerMergeCandidateFinderTest < Test::Unit::TestCase
  FakePlayer = Struct.new(:name, :full_name)

  def test_similarity_score_uses_best_name_combination
    fuzzy = Struct.new(:scores) do
      def getDistance(left, right)
        scores.fetch([left, right], 0)
      end
    end.new({
      ['Ana', 'Ana Clara'] => 1.1,
      ['Ana', 'Maria Silva'] => 0.6,
      ['Ana Maria', 'Ana Clara'] => 1.9,
      ['Ana Maria', 'Maria Silva'] => 1.2,
    })

    finder = PlayerMergeCandidateFinder.new(Player.none, fuzzy)

    left = FakePlayer.new('Ana', 'Ana Maria')
    right = FakePlayer.new('Ana Clara', 'Maria Silva')

    assert_in_delta 1.9, finder.similarity_score(left, right), 0.001
  end

  def test_suggest_threshold_uses_biggest_gap
    finder = PlayerMergeCandidateFinder.new(Player.none)
    candidates = [2.7, 2.6, 2.55, 1.8, 1.75].map { |score| { score: score } }

    assert_in_delta 1.8, finder.suggest_threshold(candidates), 0.001
  end
end
