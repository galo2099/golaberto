module ChampionshipHelper
  class TeamCampaign
    attr_reader :points, :games, :wins, :draws, :losses,
                :goals_for, :goals_against, :goals_pen, :goals_away,
                :last_game, :bias, :add_sub

    def initialize(team_group)
      @games = 0
      @points = 0
      @wins = 0
      @draws = 0
      @losses = 0
      @goals_for = 0
      @goals_against = 0
      @goals_pen = 0
      @goals_away = 0
      @team_group = team_group
      @team_id = team_group.team_id
      @last_game = nil
      @points += team_group.add_sub
      @bias = team_group.bias
    end

    def add_game(game)
      if (game.home_id != @team_id and game.away_id != @team_id)
        return
      end
      championship = game.phase.championship
      points_for_win = championship.point_win
      points_for_draw = championship.point_draw
      points_for_loss = championship.point_loss
      bonus = game.phase.bonus_points
      bonus_threshold = game.phase.bonus_points_threshold
      home_score = game.home_score
      away_score = game.away_score
      home_id = game.home_id
      @games += 1
      if (home_id == @team_id) then
        @goals_pen += game.home_pen unless game.home_pen.nil?
      else
        @goals_pen += game.away_pen unless game.away_pen.nil?
        @goals_away += away_score
      end
      if home_score > away_score then
        if (home_id == @team_id) then 
          @wins += 1
          @points += points_for_win
          if home_score - away_score >= bonus_threshold then
            @points += bonus
          end
        else
          @losses += 1
          @points += points_for_loss
        end
      elsif home_score < away_score then
        if (home_id == @team_id) then 
          @losses += 1
          @points += points_for_loss
        else
          @wins += 1
          @points += points_for_win
          if away_score - home_score >= bonus_threshold then
            @points += bonus
          end
        end
      else
        @draws += 1
        @points += points_for_draw
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
      Game.find(:first, :conditions => [ "(home_id = ? OR away_id = ?) AND phase_id = ? AND played = ?", @team_id, @team_id, @team_group.group.phase, false ], :order => :date)
    end
  end
end
