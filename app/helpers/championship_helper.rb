module ChampionshipHelper
  class TeamCampaign
    attr_reader :points, :games, :wins, :draws, :losses,
                :goals_for, :goals_against, :goals_aet, :goals_pen,
                :goals_away, :last_game, :bias, :add_sub, :team_group,
                :promoted_odds, :relegated_odds, :first_odds, :name, :home_games

    def initialize(team_group)
      @games = 0
      @points = 0
      @wins = 0
      @draws = 0
      @losses = 0
      @goals_for = 0
      @goals_against = 0
      @goals_aet = 0
      @goals_pen = 0
      @goals_away = 0
      @team_group = team_group
      @team_id = team_group.team_id
      @name = team_group.team.name
      @last_game = nil
      @points += team_group.add_sub
      @bias = team_group.bias
      @promoted_odds = team_group.promoted_odds
      @relegated_odds = team_group.relegated_odds
      @first_odds = team_group.first_odds
      championship = team_group.group.phase.championship
      @points_for_win = championship.point_win
      @points_for_draw = championship.point_draw
      @points_for_loss = championship.point_loss
      @bonus = team_group.group.phase.bonus_points
      @bonus_threshold = team_group.group.phase.bonus_points_threshold
      @home_games = Hash.new
    end

    def add_game_score_only(home_id, away_id, home_score, away_score)
      if (home_id != @team_id and away_id != @team_id)
        return
      end
      is_home = home_id == @team_id
      if is_home then
        @home_games[away_id] = [ home_score, away_score ]
      end
      @games += 1
      if home_score > away_score then
        if is_home then
          @wins += 1
          @points += @points_for_win
          if home_score - away_score >= @bonus_threshold then
            @points += @bonus
          end
        else
          @losses += 1
          @points += @points_for_loss
        end
      elsif home_score < away_score then
        if is_home then
          @losses += 1
          @points += @points_for_loss
        else
          @wins += 1
          @points += @points_for_win
          if away_score - home_score >= @bonus_threshold then
            @points += @bonus
          end
        end
      else
        @draws += 1
        @points += @points_for_draw
      end
      if is_home then
        @goals_for += home_score
        @goals_against += away_score
      else
        @goals_against += home_score
        @goals_for += away_score
        @goals_away += away_score
      end
    end

    def add_game(game)
      if (game.home_id != @team_id and game.away_id != @team_id)
        return
      end
      home_score = game.home_score
      away_score = game.away_score
      home_id = game.home_id
      @games += 1
      if (home_id == @team_id) then
        @home_games[game.away_id] = [ home_score, away_score ]
        @goals_aet += game.home_aet unless game.home_aet.nil?
        @goals_pen += game.home_pen unless game.home_pen.nil?
      else
        @goals_aet += game.away_aet unless game.away_aet.nil?
        @goals_pen += game.away_pen unless game.away_pen.nil?
        @goals_away += away_score
      end
      if home_score > away_score then
        if (home_id == @team_id) then
          @wins += 1
          @points += @points_for_win
          if home_score - away_score >= @bonus_threshold then
            @points += @bonus
          end
        else
          @losses += 1
          @points += @points_for_loss
        end
      elsif home_score < away_score then
        if (home_id == @team_id) then
          @losses += 1
          @points += @points_for_loss
        else
          @wins += 1
          @points += @points_for_win
          if away_score - home_score >= @bonus_threshold then
            @points += @bonus
          end
        end
      else
        @draws += 1
        @points += @points_for_draw
      end
      if (home_id == @team_id) then
        @goals_for += home_score
        @goals_against += away_score
      else
        @goals_against += home_score
        @goals_for += away_score
      end
      @last_game = game
    end

    def goals_diff
      @goals_for - @goals_against
    end

    def goals_avg
      @goals_for / (@goals_against + 0.0000001)
    end

    def next_game
      Game.find(:first, :conditions => [ "(home_id = ? OR away_id = ?) AND phase_id = ? AND played = ? AND date >= ?", @team_id, @team_id, @team_group.group.phase, false, Date.today ], :order => :date)
    end
  end
end
