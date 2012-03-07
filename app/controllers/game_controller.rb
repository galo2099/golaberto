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
    items_per_page = 30
    @type = params[:type] || :scheduled
    if (@type == :scheduled)
      conditions = [ "played = ?", false ]
      order = "date ASC, phase_id, time ASC"
      post_sort = proc do |g|
        [(g.date.to_time.to_i), g.phase_id, (g.time.to_i), g.home.name]
      end
      default_start = (Date.today - 7.days)
    else
      conditions = [ "played = ?", true ]
      order = "date DESC, phase_id, time DESC"
      post_sort = proc do |g|
        [-(g.date.to_time.to_i), g.phase_id, -(g.time.to_i), g.home.name]
      end
      default_end = Date.today
    end

    @categories = Category.all
    @category = params[:category] || 1
    @category = @category.to_i
    phases = Phase.joins(:championship).where(:championships => { :category_id => @category })
    conditions[0] << " AND phase_id IN (?)"
    conditions << phases

    @date_range_start = params[:date_range_start] || default_start
    @date_range_end = params[:date_range_end] || default_end
    unless @date_range_start.nil? or @date_range_start.to_date.nil?
      conditions[0] << " AND date >= ?"
      conditions << @date_range_start.to_date
    end
    unless @date_range_end.nil? or @date_range_end.to_date.nil?
      conditions[0] << " AND date <= ?"
      conditions << @date_range_end.to_date
    end

    @games = Game.paginate :order => order,
                           :conditions => conditions,
                           :page => params[:page]
    @sorted_games = @games.sort_by{|g|post_sort.call(g)}
  end

  def destroy
    game = Game.find(params["id"]).destroy
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
    @game = @phase.games.build(params["game"])
    @game.date = Date.today
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

    # work around rails TIME bug
    unless params[:game]["time(4i)"].to_s.empty? and params[:game]["time(5i)"].to_s.empty?
      params[:game]["time(1i)"] = "2000"
      params[:game]["time(2i)"] = "1"
      params[:game]["time(3i)"] = "1"
    end

    @game.attributes = params["game"]
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
    puts @goals

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
    items_per_page = 10
    @partial = params[:partial]
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @game = Game.find(params["game"])
    @team = Team.find(params["team"])
    @home_away = @game.home.id == @team.id ? "home" : "away"
    @total = Player.count :conditions => conditions
    @players = Player.order(:name).where(conditions).paginate(:per_page => items_per_page, :page => params[:page])
  end

  def create_stadium_for_edit
    @stadium = Stadium.new(params[:stadium])
    if @stadium.save
      @stadiums = Stadium.order(:name)
      render :inline => "<option value=""></option><%= options_from_collection_for_select @stadiums, 'id', 'name', @stadium.id %>"
    else
      render :nothing => true, :status => 401
    end
  end

  def create_referee_for_edit
    @referee = Referee.new(params[:referee])
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
    unless params[:name].to_s.empty?
      player = Player.new(:name => params[:name])
      if player.save
        params[:team_player][:player_id] = player.id
      else
        params[:team_player][:player_id] = nil
      end
    end
    team_player = TeamPlayer.new(params["team_player"])
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
    @game.player_games = params[:player_game].values.map{|v| PlayerGame.new(v)}
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
        goal = Goal.new(goal_params)
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
    @home_players = @game.home.team_players.find(
      :all,
      :order => "players.name",
      :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
    @away_players = @game.away.team_players.find(
      :all,
      :order => "players.name",
      :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
  end

  def prepare_for_edit_squad
    @home_squad = @game.home_player_games
    @away_squad = @game.away_player_games
    @home_players.map!{|p| PlayerGame.new({ :game => @game, :team => @game.home, :player => p }) }
    @away_players.map!{|p| PlayerGame.new({ :game => @game, :team => @game.away, :player => p }) }
    @home_players = @home_players.select{|p| @home_squad.select{|pg| pg.player.id == p.player.id}.size == 0}
    @away_players = @away_players.select{|p| @away_squad.select{|pg| pg.player.id == p.player.id}.size == 0}

    @home_squad.sort!
    @away_squad.sort!
  end
end
