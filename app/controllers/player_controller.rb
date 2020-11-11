class PlayerController < ApplicationController
  include ApplicationHelper

  N_("Player")

  authorize_resource

  def index
    list
    render :action => :list
  end

  def destroy_team
    team_player = TeamPlayer.find(params[:id])
    goals = team_player.player.goals.where(team_id: team_player.team_id)
    goals.each do |g|
      if g.game.phase.championship_id == team_player.championship_id
        g.destroy
      end
    end
    player_games = team_player.player.player_games.where(team_id: team_player.team_id)
    player_games.each do |pg|
      if pg.game.phase.championship_id == team_player.championship_id
        pg.destroy
      end
    end
    team_player.destroy
    render :nothing => true
  end

  def list
    @id = params[:id]
    @country = params[:country]
    @continent = params[:continent] || ""
    @players = Player.order("rating DESC")
    @players = @players.where(["name LIKE ?", "%#{@id}%"]) unless @id.nil?

    countries_with_players = { @country => 0 }
    countries_with_players.merge!(@players.group(:country).size)

    countries_found = []
    countries_not_found = []
    golaberto_options_for_country_select.each do |translated_country, original_country|
      count = countries_with_players[original_country]
      if count.nil? then
        countries_not_found << [translated_country, original_country]
      else
        countries_found << [translated_country + " (#{count})", original_country]
      end
    end

    @country_list = [[s_("Country|All") + " (#{@players.size})", ""]] + countries_found + countries_not_found
    @countries = {}
    @countries[""] = @country_list
    ApplicationHelper::Continent::ALL.each do |name, c|
      @countries[name] = [[s_("Country|All") + " (#{@players.where(country: c.countries.map{|c|c.name}).size})", ""]] + @country_list.select{|_, n| ApplicationHelper::Continent.country_to_continent[n] == c}
    end

    unless @continent.blank?
      @players = @players.where(country: Continent::ALL[@continent].countries.map{|c|c.name})
      unless Continent::ALL[@continent].countries.map{|c|c.name}.include? @country
        @country = nil
      end
    end

    @players = @players.page(params[:page])
    @players = @players.where(country: @country) unless @country.blank?
    if @players.size == 1
      redirect_to :action => :show, :id => @players.first
    end
  end

  def show
    store_location
    @player = Player.find(params["id"])
  end

  def edit
    @player = Player.find(params["id"])
  end

  def update
    @player = Player.find(params[:id])
    @player.attributes = player_params

    if @player.save
      flash[:notice] = _("Player was successfully updated")
      redirect_to :action => :show, :id => @player
    else
      render :action => :edit
    end
  end

  def update_rating
    starting = Time.now
    games = ActiveRecord::Base.connection.exec_query("SELECT `games`.`id`, `games`.`date`, `games`.`home_score`, `games`.`away_score`, `games`.`home_aet`, `games`.`away_aet`, `games`.`home_field`, `games`.`home_id`, `games`.`away_id` FROM `games` INNER JOIN `phases` ON `phases`.`id` = `games`.`phase_id` INNER JOIN `championships` ON `championships`.`id` = `phases`.`championship_id` WHERE (date > '2016-11-10') AND `games`.`played` = 1 AND `championships`.`category_id` = 1").rows.map {|x| {id: x[0], date: x[1], home_score: x[2], away_score: x[3], home_aet: x[4], away_aet: x[5], home_field: x[6], home_id: x[7], away_id: x[8]}}
    teams = Team.all.pluck(:id, :off_rating, :def_rating).inject({}) {|res,x| res[x[0]] = {off_rating: x[1], def_rating: x[2]}; res}
    goals = ActiveRecord::Base.connection.exec_query("select id, player_id, game_id, team_id, time, penalty, own_goal, aet from goals where game_id in (#{games.map{|g|g[:id]}.join(",")})").rows.inject(Hash.new{|h,k| h[k] = []}) {|res,x| res[x[2]].append({id: x[0], player_id: x[1], game_id: x[2], team_id: x[3], time: x[4], penalty: x[5], own_goal: x[6], aet: x[7]}); res}
    player_games = ActiveRecord::Base.connection.exec_query("select player_id, game_id, team_id, `on`, off, yellow, red, players.position from player_games INNER JOIN `players` ON `players`.`id` = `player_games`.`player_id` where off > 0 and game_id in (#{games.map{|g|g[:id]}.join(",")})").rows.inject(Hash.new{|h,k| h[k] = []}) {|res,x| res[x[1]].append({player_id: x[0], game_id: x[1], team_id: x[2], on: x[3], off: x[4], yellow: x[5], red: x[6], position: x[7]}); res}
    p Time.now - starting

    starting = Time.now
    players = player_ranking(games, teams, goals, player_games)
    p Time.now - starting

    now = Time.zone.now.to_s.chop.chop.chop.chop
    sql = "INSERT INTO players (id,off_rating,def_rating,rating,created_at,updated_at) VALUES "
    players.each do |k,v|
      sql << "(#{k}, #{v ? v[:off] / [v[:minutes], 1].max * 90 : "NULL"}, #{v ? v[:def] / [v[:minutes], 1].max * 90 : "NULL"}, #{v ? (v[:off] + v[:def]) / (v[:minutes] + 900) * 90 : "NULL"}, '#{now}', '#{now}'),"
    end
    sql.chop!
    sql << "ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at);"
    ActiveRecord::Base.connection.execute(sql)
    redirect_to :back
  end

  def new
    @player = Player.new
  end

  def create
    @player = Player.new(player_params)
    if @player.save
      flash[:notice] = _("Player was successfully created")
      redirect_to :action => :show, :id => @player
    else
      render :action => :new
    end
  end

  def destroy
    Player.find(params[:id]).destroy
    redirect_to :action => :list
  end

  private

  def player_params
    params.require(:player).permit("name", "position", "birth", "country", "full_name")
  end

  def squash_date(timestamp, now)
    x = (timestamp-now).to_f / (730 * 24 * 60 * 60)
    return 1 + (Math.exp(x)-Math.exp(-x))/(Math.exp(x)+Math.exp(-x))
  end

  def player_ranking(games, teams, goals, player_games)
    starting = Time.now
    players = Player.all.pluck(:id).map{|p| [p, {off: 0.0, def: 0.0, minutes: 0}]}.to_h
    p Time.now - starting
    now = DateTime.now.to_i

    games.each do |g|
      next if player_games[g[:id]].size == 0
      home_adv = if g[:home_field] == Game.home_fields[:left] then Game::HOME_ADV elsif g[:home_field] == Game.home_fields[:neutral] then 0.0 else -Game::HOME_ADV end
      weight = squash_date(g[:date].to_i, now)

      length = g[:home_aet].nil? ? 90 : 120

  #    p (0-(teams[g[:away_id]][:def_rating]+home_adv))/((teams[g[:away_id]][:def_rating]+home_adv)*0.424+0.548)*(Game::AVG_BASE*0.424+0.548)
  #    p 1 / ((teams[g[:home_id]][:def_rating]+home_adv)*0.424+0.548) * (Game::AVG_BASE*0.424+0.548)

      home_for_zero_per_90 = (0-(teams[g[:away_id]][:def_rating]+home_adv))/((teams[g[:away_id]][:def_rating]+home_adv)*0.424+0.548)*(Game::AVG_BASE*0.424+0.548) / length / 11
      home_for_goal_weight = 1 / ((teams[g[:away_id]][:def_rating]+home_adv)*0.424+0.548) * (Game::AVG_BASE*0.424+0.548)
      home_agg_zero_per_90 = (0-(teams[g[:away_id]][:off_rating]-home_adv))/((teams[g[:away_id]][:off_rating]-home_adv)*0.424+0.548)*(Game::AVG_BASE*0.424+0.548) / length / 11 * -1
      home_agg_goal_weight = 1 / ((teams[g[:away_id]][:off_rating]-home_adv)*0.424+0.548) * (Game::AVG_BASE*0.424+0.548) * -1

      away_for_zero_per_90 = (0-(teams[g[:home_id]][:def_rating]-home_adv))/((teams[g[:home_id]][:def_rating]-home_adv)*0.424+0.548)*(Game::AVG_BASE*0.424+0.548) / length / 11
      away_for_goal_weight = 1 / ((teams[g[:home_id]][:def_rating]-home_adv)*0.424+0.548) * (Game::AVG_BASE*0.424+0.548)
      away_agg_zero_per_90 = (0-(teams[g[:home_id]][:off_rating]+home_adv))/((teams[g[:home_id]][:off_rating]+home_adv)*0.424+0.548)*(Game::AVG_BASE*0.424+0.548) / length / 11 * -1
      away_agg_goal_weight = 1 / ((teams[g[:home_id]][:off_rating]+home_adv)*0.424+0.548) * (Game::AVG_BASE*0.424+0.548) * -1

      game_players = player_games[g[:id]].map{|p| [p[:player_id], {off: 0.0, def: 0.0, minutes: 0}]}.to_h

      home_players = player_games[g[:id]].select{|pg| pg[:team_id] == g[:home_id] and pg[:off] > 0}
      away_players = player_games[g[:id]].select{|pg| pg[:team_id] == g[:away_id] and pg[:off] > 0}

      home_goals = goals[g[:id]].select{|go| (go[:team_id] == g[:away_id] and go[:own_goal] == 1) or (go[:team_id] == g[:home_id] and go[:own_goal] == 0)}
      away_goals = goals[g[:id]].select{|go| (go[:team_id] == g[:home_id] and go[:own_goal] == 1) or (go[:team_id] == g[:away_id] and go[:own_goal] == 0)}

      intervals = home_players.map{|p| [p[:on], p[:off]]}.inject([[]]) {|res, n| res.map{|x| (x + n).sort.uniq }}.flatten(1)
      intervals.each_cons(2) do |from, to|
        pos = {}
        pos.default = 0
        hp = home_players.select{|p| [from, p[:on]].max < [to, p[:off]].min}
        hp.each{|p| pos[p[:position]] += 1}
        off_w = hp.size / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[nil])
        off_w = { "g" => off_w * 0.3, "dc" => off_w * 0.7, "cm" => off_w, "fw" => off_w * 1.0, nil => off_w }
        def_w = hp.size / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[nil])
        def_w = { "g" => def_w * 4.0, "dc" => def_w * 2.0, "cm" => def_w, "fw" => def_w * 0.5, nil => def_w }
        home_goals_interval = home_goals.select{|go| go[:time] > from and go[:time] <= to }
        away_goals_interval = away_goals.select{|go| go[:time] > from and go[:time] <= to }
        home_goals_own = home_goals_interval.select{|go| go[:own_goal] == 1 and go[:penalty] == 0}.size
        home_goals_regular = home_goals_interval.select{|go| go[:own_goal] == 0 and go[:penalty] == 0}.size
        home_goals_penalty = home_goals_interval.select{|go| go[:own_goal] == 0 and go[:penalty] == 1}.size
        hp.each do |p|
          minutes = (to - from)
          game_players[p[:player_id]][:minutes] += minutes * weight
          off_player_weight = off_w[p[:position]]
          game_players[p[:player_id]][:off] +=
              (minutes * home_for_zero_per_90 * off_player_weight +
               home_goals_own * home_for_goal_weight * off_player_weight / hp.size +
               home_goals_regular * home_for_goal_weight * off_player_weight / hp.size / 4 * 3 +
               home_goals_penalty * home_for_goal_weight * off_player_weight / hp.size / 6 * 5 +
               home_goals_interval.select{|go| go[:player_id] == p[:player_id] && go[:penalty] == 0}.size.to_f * home_for_goal_weight / 4 +
               home_goals_interval.select{|go| go[:player_id] == p[:player_id] && go[:penalty] == 1}.size.to_f * home_for_goal_weight / 6) * weight
          game_players[p[:player_id]][:def] += (minutes * home_agg_zero_per_90 + away_goals_interval.size * home_agg_goal_weight / hp.size) * def_w[p[:position]] * weight
        end
      end

      intervals = away_players.map{|p| [p[:on], p[:off]]}.inject([[]]) {|res, n| res.map{|x| (x + n).sort.uniq }}.flatten(1)
      intervals.each_cons(2) do |from, to|
        pos = {}
        pos.default = 0
        ap = away_players.select{|p| [from, p[:on]].max < [to, p[:off]].min}
        ap.each{|p|pos[p[:position]] += 1}
        off_w = ap.size / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[nil])
        off_w = { "g" => off_w * 0.3, "dc" => off_w * 0.7, "cm" => off_w, "fw" => off_w * 1.0, nil => off_w }
        def_w = ap.size / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[nil])
        def_w = { "g" => def_w * 4.0, "dc" => def_w * 2.0, "cm" => def_w, "fw" => def_w * 0.5, nil => def_w }
        home_goals_interval = home_goals.select{|go| go[:time] > from and go[:time] <= to }
        away_goals_interval = away_goals.select{|go| go[:time] > from and go[:time] <= to }
        away_goals_own = away_goals_interval.select{|go| go[:own_goal] == 1 and go[:penalty] == 0}.size
        away_goals_regular = away_goals_interval.select{|go| go[:own_goal] == 0 and go[:penalty] == 0}.size
        away_goals_penalty = away_goals_interval.select{|go| go[:own_goal] == 0 and go[:penalty] == 1}.size
        ap.each do |p|
          minutes = (to - from)
          game_players[p[:player_id]][:minutes] += minutes * weight
          off_player_weight = off_w[p[:position]]
          game_players[p[:player_id]][:off] +=
              (minutes * away_for_zero_per_90 * off_player_weight +
               away_goals_own * away_for_goal_weight * off_player_weight / ap.size +
               away_goals_regular * away_for_goal_weight * off_player_weight / ap.size / 4 * 3 +
               away_goals_penalty * away_for_goal_weight * off_player_weight / ap.size / 6 * 5 +
               away_goals_interval.select{|go| go[:player_id] == p[:player_id] && go[:penalty] == 0}.size.to_f * away_for_goal_weight / 4 +
               away_goals_interval.select{|go| go[:player_id] == p[:player_id] && go[:penalty] == 1}.size.to_f * away_for_goal_weight / 6) * weight
          game_players[p[:player_id]][:def] += (minutes * away_agg_zero_per_90 + home_goals_interval.size * away_agg_goal_weight / ap.size) * def_w[p[:position]] * weight
        end
      end

  #    p g
  #    p home_players.map{|x| [x[:player_id], game_players[x[:player_id]]] }.to_h
  #    p away_players.map{|x| [x[:player_id], game_players[x[:player_id]]] }.to_h
  #    p home_players.map{|x| game_players[x[:player_id]][:off] }.sum
  #    p home_players.map{|x| game_players[x[:player_id]][:def] }.sum
  #    p away_players.map{|x| game_players[x[:player_id]][:off] }.sum
  #    p away_players.map{|x| game_players[x[:player_id]][:def] }.sum

      game_players.each do |id, data|
        data.each do |k, v|
          players[id][k] += v
        end
      end
    end

    return players
  end
end
