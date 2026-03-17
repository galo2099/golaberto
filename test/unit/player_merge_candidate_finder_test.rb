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

  def test_find_candidates_only_includes_complementary_provider_ids
    birth = Date.new(2000, 1, 1)
    left = Player.create!(name: 'Left', country: 'Brazil', birth: birth, sofascore_id: 'sofa-1', soccerway_id: nil)
    right = Player.create!(name: 'Right', country: 'Brazil', birth: birth, sofascore_id: nil, soccerway_id: 'sw-1')
    ignored = Player.create!(name: 'Ignored', country: 'Brazil', birth: birth, sofascore_id: 'sofa-2', soccerway_id: 'sw-2')

    finder = PlayerMergeCandidateFinder.new(Player.where(id: [left.id, right.id, ignored.id]))
    candidates = finder.find_candidates

    assert_equal 1, candidates.size
    assert_equal [left.id, right.id].sort, [candidates.first[:left].id, candidates.first[:right].id].sort
    refute_includes [candidates.first[:left].id, candidates.first[:right].id], ignored.id
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
