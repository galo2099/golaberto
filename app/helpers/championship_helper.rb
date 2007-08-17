module ChampionshipHelper
  class TeamCampaign
    attr_reader :points, :games, :wins, :draws, :losses,
                :goals_for, :goals_against, :goals_pen, :goals_away,
                :last_game, :next_game, :bias, :add_sub

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
      @last_game = nil
      @next_game = nil
      @team = team_group.team
      @points += team_group.add_sub
      @bias = team_group.bias
    end

    def add_game(game)
      if (game.home_id != @team.id and game.away_id != @team.id)
        return
      end
      championship = game.phase.championship
      points_for_win = championship.point_win
      points_for_draw = championship.point_draw
      points_for_loss = championship.point_loss
      @games += 1
      if (game.home_id == @team.id) then 
        @goals_pen += game.home_pen unless game.home_pen.nil?
      else
        @goals_pen += game.away_pen unless game.away_pen.nil?
        @goals_away += game.away_score
      end
      if game.home_score > game.away_score then
        if (game.home_id == @team.id) then 
          @wins += 1
          @points += points_for_win
        else
          @losses += 1
          @points += points_for_loss
        end
      elsif game.home_score < game.away_score then
        if (game.home_id == @team.id) then 
          @losses += 1
          @points += points_for_loss
        else
          @wins += 1
          @points += points_for_win
        end
      else
        @draws += 1
        @points += points_for_draw
      end
      if (game.home_id == @team.id) then
        @goals_for += game.home_score
        @goals_against += game.away_score
      else
        @goals_against += game.home_score
        @goals_for += game.away_score
      end
      @last_game = game
    end

    def goals_diff
      @goals_for - @goals_against 
    end

    def goals_avg
      @goals_for / (@goals_against + 0.0000001)
    end
  end
end
