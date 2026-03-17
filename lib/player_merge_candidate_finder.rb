require 'fuzzy/fuzzy'

class PlayerMergeCandidateFinder
  DEFAULT_RECENT_MATCHES = 5
  NAME_WEIGHT = 1.0
  FULL_NAME_WEIGHT = 2.0
  FULL_NAME_MATCH_COVERAGE = 0.9

  def initialize(scope = Player.all, fuzzy_match = FuzzyTeamMatch.new)
    @scope = scope
    @fuzzy_match = fuzzy_match
  end

  def find_candidates
    grouped_players.values.flat_map do |players|
      players.combination(2).map do |left, right|
        { left: left, right: right, score: similarity_score(left, right) }
      end
    end.sort_by { |candidate| -candidate[:score] }
  end

  def suggest_threshold(candidates)
    full_name_threshold = threshold_for_full_name_matches(candidates)
    return full_name_threshold unless full_name_threshold.nil?

    gap_based_threshold(candidates)
  end

  def threshold_for_full_name_matches(candidates, coverage = FULL_NAME_MATCH_COVERAGE)
    full_name_scores = full_name_match_candidates(candidates).map { |candidate| candidate[:score] }.sort.reverse
    return nil if full_name_scores.empty?

    index = [(full_name_scores.size * coverage).ceil - 1, full_name_scores.size - 1].min
    full_name_scores[index]
  end

  def full_name_match_candidates(candidates)
    candidates.select { |candidate| exact_full_name_match?(candidate[:left], candidate[:right]) }
  end

  def similarity_score(left, right)
    name_score = best_distance(left.name, right.name) * NAME_WEIGHT
    full_name_score = best_distance(left.full_name, right.full_name) * FULL_NAME_WEIGHT

    name_score + full_name_score
  end

  def best_distance(left_value, right_value)
    return 0.0 if left_value.blank? or right_value.blank?

    @fuzzy_match.getDistance(left_value, right_value).to_f
  end

  def recent_matches(player, limit = DEFAULT_RECENT_MATCHES)
    player.player_games
          .includes(:team, game: [:home, :away])
          .joins(:game)
          .order('games.date desc')
          .limit(limit)
          .map do |player_game|
      game = player_game.game
      opponent = game.home_id == player_game.team_id ? game.away : game.home
      {
        date: game.date,
        team_name: player_game.team&.name,
        opponent_name: opponent&.name,
        score: "#{game.home_score}-#{game.away_score}",
      }
    end
  end

  private

  def gap_based_threshold(candidates)
    scores = candidates.map { |candidate| candidate[:score] }
    return 0 if scores.empty?
    return scores.first if scores.size == 1

    best_gap = -Float::INFINITY
    best_threshold = scores.last

    scores.each_cons(2) do |high, low|
      gap = high - low
      if gap > best_gap
        best_gap = gap
        best_threshold = low
      end
    end

    best_threshold
  end

  def exact_full_name_match?(left, right)
    return false if left.full_name.blank? or right.full_name.blank?

    normalize_name(left.full_name) == normalize_name(right.full_name)
  end

  def normalize_name(name)
    ActiveSupport::Inflector.transliterate(name).downcase.squish
  end

  def grouped_players
    @scope.where.not(country: nil)
          .where.not(birth: nil)
          .where.not(name: [nil, ''])
          .order(:country, :birth)
          .group_by { |player| [player.country, player.birth] }
          .select { |_, players| players.size > 1 }
  end
end
