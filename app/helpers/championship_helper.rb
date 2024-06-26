module ChampionshipHelper
  def digest_cache_key(collection)
    collection.inject(Digest::SHA256.new){|digest,obj| digest << obj.cache_key}.hexdigest
  end

  def region_flag_url(region, name)
    filename = ""
    if region == "national" then
      filename = name.parameterize(separator: '_')
    else
      if region == "world" then
        filename = "fifa"
      else
        case name
        when "Africa"
          filename = "caf"
        when "Asia"
          filename = "afc"
        when "North/Central America & Caribbean"
          filename = "concacaf"
        when "Europe"
          filename = "uefa"
        when "Oceania"
          filename = "ofc"
        when "South America"
          filename = "conmebol"
        end
      end
    end
    "#{Rails.configuration.golaberto_image_url_prefix}/countries/flags/#{filename}_15.png"
  end

  def championship_name(champ, include_region = true, include_season = true, params)
    prefix = link_to(image_tag(region_flag_url(champ.region, champ.region_name) , :title => _(champ.region_name), size: "15x15"), params) + link_to(("&nbsp;" * 6).html_safe, params, class: "championship_name_spacer")
    content_tag :div, class: "championship_name" do
      prefix.html_safe + link_to(champ.full_name(include_region, include_season), params)
    end
  end

  class TeamCampaign
    attr_reader :points, :games, :wins, :draws, :losses,
                :goals_for, :goals_against, :goals_aet, :goals_pen,
                :goals_away, :last_game, :bias, :add_sub, :team_group,
                :zones, :name, :home_games, :form

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
      @home_games = Hash.new
      @form = Array.new
      @last_game = nil
      @team_group = team_group
      @team_id = team_group.team_id
      @name = team_group.team.name
      @points += team_group.add_sub
      @bias = team_group.bias
      @odds = team_group.odds
      championship = team_group.group.phase.championship
      @points_for_win = championship.point_win
      @points_for_draw = championship.point_draw
      @points_for_loss = championship.point_loss
      @bonus = team_group.group.phase.bonus_points
      @bonus_threshold = team_group.group.phase.bonus_points_threshold
    end

    def cache_key
      sha256 = Digest::SHA256.new
      [ @games, @points, @wins, @draws, @losses, @goals_for, @goals_against,
        @goals_aet, @goals_pen, @goals_away, @home_games ].each do |var|
        sha256 << var.to_param
      end
      sha256 << @last_game.cache_key if @last_game
      sha256.hexdigest
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
          @form.push("w")
          @points += @points_for_win
          if home_score - away_score >= @bonus_threshold then
            @points += @bonus
          end
        else
          @losses += 1
          @form.push("l")
          @points += @points_for_loss
        end
      elsif home_score < away_score then
        if (home_id == @team_id) then
          @losses += 1
          @form.push("l")
          @points += @points_for_loss
        else
          @wins += 1
          @form.push("w")
          @points += @points_for_win
          if away_score - home_score >= @bonus_threshold then
            @points += @bonus
          end
        end
      else
        @draws += 1
        @form.push("d")
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
      Game.where("(home_id = ? OR away_id = ?) AND phase_id = ? AND played = ? AND date >= ?", @team_id, @team_id, @team_group.group.phase, false, Date.today)
          .includes(:home, :away).order(:date).first
    end
  end
end
