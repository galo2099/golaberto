require 'fuzzy/fuzzy'

class PlayerMergeCandidateFinder
  DEFAULT_RECENT_MATCHES = 5

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

  def similarity_score(left, right)
    name_score = best_distance(left.name, right.name)
    full_name_score = best_distance(left.full_name, right.full_name)

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

  def grouped_players
    @scope.where.not(country: nil, birth: nil)
          .where.not(name: [nil, ''])
          .order(:country, :birth)
          .group_by { |player| [player.country, player.birth] }
          .select { |_, players| players.size > 1 }
  end

end
