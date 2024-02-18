class GameController < ApplicationController
  N_("Game")

  authorize_resource

  def show
    @game = Game.find(params["id"])
    @last_games = @game.find_n_previous_games_by_team_versus_team(5)
  end

  def index
    redirect_to :action => :list
  end

  def list
    store_location
    @type = params[:type].to_sym || :scheduled
    @games = Game
    @categories = Category.all
    @category = params[:category] || 1
    @category = @category.to_i
    @games = @games.joins(phase: :championship).where(championships: { category_id: @category }, played: @type != :scheduled)

    @min, @max = pagy_calendar_period(@games)
    if not params[:week] then
      params[:week_page] = ((Date.today + 2 - @min.to_date) / 7 + 1).to_i
    else
      params[:week_page] = ((params[:week].to_date + 2 - @min.to_date) / 7 + 1).to_i
    end
    @calendar, @pagy, @games = pagy_calendar(@games, week: {}, pagy: { items: 30 })
    params.delete(:week_page)

    if @type == :scheduled
      @games = @games.order(Arel.sql("DATE(IF(has_time, CONVERT_TZ(date, '+00:00', '#{ActiveSupport::TimeZone.seconds_to_utc_offset cookie_timezone.now.utc_offset}'), date)) ASC, phase_id, date ASC"))
      post_sort = proc do |g|
        [if g.has_time then g.date.in_time_zone(cookie_timezone).to_date.to_datetime.to_i else g.date.to_date.to_datetime.to_i end,
          g.phase_id, g.date.to_i, g.home.name]
      end
    else
      @games = @games.order(Arel.sql("DATE(IF(has_time, CONVERT_TZ(date, '+00:00', '#{ActiveSupport::TimeZone.seconds_to_utc_offset cookie_timezone.now.utc_offset}'), date)) DESC, phase_id, date DESC"))
      post_sort = proc do |g|
        [if g.has_time then -g.date.in_time_zone(cookie_timezone).to_date.to_datetime.to_i else -g.date.to_date.to_datetime.to_i end,
          g.phase_id, -g.date.to_i, g.home.name]
      end
    end

    @sorted_games = @games.includes(:home, :away, :phase, :championship).sort_by{|g|post_sort.call(g)}
  end

  def destroy
    Game.find(params["id"]).destroy
    redirect_back_or_default :back
  end

  def edit
    begin
      @game = Game.find(params["id"])
      @redirect = params[:redirect]
      prepare_for_edit
      if request.xml_http_request?
        players = @home_players
        team = @game.home
        home_away = "home"
        if params["home_away"] == "away"
          players = @away_players
          team = @game.away
          home_away = "away"
        end
        render :partial => "player_list",
               :locals => { :players => players,
                            :team => team,
                            :game => @game,
                            :home_away => home_away }
      end
    end
  end

  def create
    @championship = Championship.find(params["championship"])
    @phase = Phase.find(params["phase"])
    @game = @phase.games.build(game_params)
    @game.date = cookie_timezone.today
    if @game.save
      redirect_to :action => :edit, :id => @game
    else
      flash[:notice] = @game.errors.full_messages
      redirect_to :controller => :championship, :action => :new_game, :id => @championship, :phase => @phase
    end
  end

  def update
    @game = Game.find(params["id"])
    game_compare = @game.dup

    @game.attributes = game_params
    @game.date = params["game_date"]
    @game.has_time = false
    unless params["hour"].empty?
      @game.date = cookie_timezone.local_to_utc(@game.date + params["hour"].to_i.hours + params["minute"].to_i.minute)
      @game.has_time = true
    end
    # set score to sane values
    @game.home_score = 0 unless @game.home_score
    @game.away_score = 0 unless @game.away_score

    all_valid = true
    saved = false

    @goals = Array.new
    all_valid &&= update_goals("home", "score", params["home_regulation_goal"])
    all_valid &&= update_goals("away", "score", params["away_regulation_goal"])
    all_valid &&= update_goals("home", "aet", params["home_aet_goal"]) if @game.home_aet
    all_valid &&= update_goals("away", "aet", params["away_aet_goal"]) if @game.away_aet

    all_valid &&= @game.valid?

    if all_valid
      @goals.sort! { |a,b| a.time <=> b.time }
      game_compare.goals = @goals
      if @game.diff(game_compare).size > 0
        @game.goals = @goals
        saved = @game.save
      else
        saved = true
      end
    end

    if saved
      flash[:notice] = _("Game saved")
      if (params["redirect"])
        redirect_to params["redirect"]
      else
        redirect_to :action => :show, :id => @game
      end
    else
      prepare_for_edit
      render :action => :edit
    end
  end

  def list_players
    @partial = params[:partial]
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @game = Game.find(params["game"])
    @team = Team.find(params["team"])
    @home_away = @game.home.id == @team.id ? "home" : "away"
    @total = Player.where(conditions).count
    @pagy, @players = pagy Player.order(:name).where(conditions)
  end

  def create_stadium_for_edit
    @stadium = Stadium.new(params.require(:stadium).permit(:name))
    if @stadium.save
      @stadiums = Stadium.order(:name)
      render :inline => "<option value=""></option><%= options_from_collection_for_select @stadiums, 'id', 'name', @stadium.id %>"
    else
      render :nothing => true, :status => 401
    end
  end

  def create_referee_for_edit
    @referee = Referee.new(params.require("referee").permit("name", "location"))
    if @referee.save
      @referees = Referee.order(:name).map do |r|
        [ "#{r.name} (#{r.location})", r.id ]
      end
      render :inline => "<option value=""></option><%= options_for_select @referees, @referee.id %>"
    else
      render :nothing => true, :status => 401
    end
  end

  def insert_team_player
    @game = Game.find(params[:id])
    @home_away = params[:home_away]
    @partial = params[:partial]
    team_player = params.require("team_player").permit("championship_id", "team_id", "player_id")
    unless params[:name].to_s.empty?
      player = Player.new(:name => params[:name])
      if player.save
        team_player[:player_id] = player.id
      else
        team_player[:player_id] = nil
      end
    end
    team_player = TeamPlayer.new(team_player)
    unless team_player.save
      render :nothing => true, :status => 401
    end
    @player = team_player.player
  end

  def edit_squad
    @game = Game.find(params[:id])
    prepare_for_edit
    prepare_for_edit_squad
  end

  def update_squad
    @game = Game.find(params[:id])
    @game.player_games = params.require(:player_game).values.map{|v| PlayerGame.new(v.permit(:player_id, :game_id, :team_id, :on, :off, :yellow, :red))}
    redirect_to :action => :show, :id => @game
  end

  private

  def update_goals(home_away, aet, goals)
    us = home_away
    them = home_away == "home" ? "away" : "home"
    all_valid = true
    @game.send(us + "_" + aet).times do |i|
      goal_params = goals[i.to_s] if goals
      unless goal_params.nil? or goal_params[:player_id].empty?
        goal = Goal.new(goal_params.permit("player_id", "time", "penalty", "own_goal", "aet"))
        goal.game_id = @game.id
        goal.team_id = goal.own_goal? ? @game.send(them + "_id") : @game.send(us + "_id")
        @goals.push goal
        all_valid &&= goal.valid?
      end
    end
    return all_valid
  end

  def prepare_for_edit
    @referees = Referee.order(:name).map do |r|
      [ "#{r.name} (#{r.location})", r.id ]
    end
    @stadiums = Stadium.order(:name)
    @selected_stadium = @game.stadium_id ? @game.stadium_id :
                        @game.version == 1 ? @game.home.stadium_id : 0
    @home_players = @game.home.team_players.order("players.name").
	    where("championship_id = ?", @game.phase.championship).map {|p| p.player}
    @away_players = @game.away.team_players.order("players.name").
      where("championship_id = ?", @game.phase.championship).map {|p| p.player}
  end

  def prepare_for_edit_squad
    @home_squad = @game.home_player_games.order("players.name")
    @away_squad = @game.away_player_games.order("players.name")
    @home_players.map!{|p| PlayerGame.new({ :game => @game, :team => @game.home, :player => p }) }
    @away_players.map!{|p| PlayerGame.new({ :game => @game, :team => @game.away, :player => p }) }
    @home_players = @home_players.select{|p| @home_squad.select{|pg| pg.player.id == p.player.id}.size == 0}
    @away_players = @away_players.select{|p| @away_squad.select{|pg| pg.player.id == p.player.id}.size == 0}
  end

  def game_params
    params.require("game").permit(
	    "home_id", "away_id", "home_score", "away_score", "home_aet", "away_aet", "home_pen",
	    "away_pen", "round", "attendance", "date", "time", "referee_id",
	    "stadium_id", "played", "hour", "minute", "home_field")
  end

  def pagy_calendar_period(collection)
#    minmax = collection.pluck('MIN(date)', 'MAX(date)').first
#    minmax = [ (minmax[0]&.in_time_zone(cookie_timezone) or DateTime.now.in_time_zone(cookie_timezone)), (minmax[1]&.in_time_zone(cookie_timezone) or (DateTime.now+1).in_time_zone(cookie_timezone)) ]
     [ ("1800-01-01".to_date).in_time_zone(cookie_timezone), ("2500-01-01".to_date).in_time_zone(cookie_timezone) ]
  end

  def pagy_calendar_filter(collection, from, to)
    collection.where(date: from...to)
  end
end
