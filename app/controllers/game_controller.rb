class GameController < ApplicationController
  before_filter :login_required, :except => [ :show ]

  def show
    @game = Game.find(@params["id"])
  end

  def index
    redirect_to :action => :list
  end

  def list
    items_per_page = 30
    conditions = { :played => true }

    @total = Game.count :conditions => conditions
    @game_pages, @games = paginate :games, :order => "date DESC, phase_id",
                                   :conditions => conditions,
                                   :per_page => items_per_page

    if request.xhr?
      render :partial => "game_list", :layout => false
    end
  end

  def destroy
    game = Game.find(@params["id"])
    options = { :controller => :championship, :action => :games, :id => game.phase.championship, :phase => game.phase }
    game.destroy
    redirect_to options
  end

  def prepare_for_edit
    @referees = Referee.find(:all, :order => :name).map do |r|
      [ "#{r.name} (#{r.location})", r.id ]
    end
    @stadiums = Stadium.find(:all, :order => :name)
    @home_players = @game.home.team_players.find(
      :all,
      :order => :name,
      :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
      @away_players = @game.away.team_players.find(
        :all,
        :order => :name,
        :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
  end

  private :prepare_for_edit

  def edit
    begin
      @game = Game.find(@params["id"])
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
    rescue
      flash[:notice] = "Game not found"
      redirect_to :action => 'list'
    end
  end

  def create
    @championship = Championship.find(@params["championship"])
    @phase = Phase.find(@params["phase"])
    @game = @phase.games.build(@params["game"])
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

    # we want to do our own date parsing
    date = params[:game].delete(:date)
    @game.date = Date.strptime(date, "%d/%m/%Y") unless date.empty?
 
    # work around rails TIME bug
    unless params[:game]["time(4i)"].to_s.empty? and params[:game]["time(5i)"].to_s.empty?
      params[:game]["time(1i)"] = "2000"
      params[:game]["time(2i)"] = "1"
      params[:game]["time(3i)"] = "1"
    end

    @game.attributes = params["game"]

    saved = @game.save

    @game.home_goals.clear
    @game.away_goals.clear

    @goals = Array.new
    @game.home_score.times do |i|
      goal = @game.home_goals.build(@params["home_goal"][i.to_s])
      if goal.player
        goal.team_id = goal.own_goal? ? @game.away_id : @game.home_id
        saved = goal.save and saved
        @goals.push goal
      end
    end
    @game.away_score.times do |i|
      goal = @game.away_goals.build(@params["away_goal"][i.to_s])
      if goal.player
        goal.team_id = goal.own_goal? ? @game.home_id : @game.away_id
        saved = goal.save and saved
        @goals.push goal
      end
    end

    if saved
      flash[:notice] = "Game saved"
      if (@params["redirect"])
        redirect_to @params["redirect"]
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
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @game = Game.find(params["game"])
    @team = Team.find(params["team"])
    @home_away = @game.home.id == @team.id ? "home" : "away"
    @total = Player.count :conditions => conditions
    @player_pages, @players = paginate :players, :order => "name",
                                       :conditions => conditions,
                                       :per_page => items_per_page
  end

  def create_stadium_for_edit
    @stadium = Stadium.new(params[:stadium])
    if @stadium.save
      @stadiums = Stadium.find(:all, :order => :name)
      render :inline => "<option value=""></option><%= options_from_collection_for_select @stadiums, 'id', 'name', @stadium.id %>"
    else
      render :nothing => true, :status => 401
    end
  end

  def create_referee_for_edit
    @referee = Referee.new(params[:referee])
    if @referee.save
      @referees = Referee.find(:all, :order => :name).map do |r|
        [ "#{r.name} (#{r.location})", r.id ]
      end
      render :inline => "<option value=""></option><%= options_for_select @referees, @referee.id %>"
    else
      render :nothing => true, :status => 401
    end
  end

  def insert_team_player
    unless params["name"].to_s.empty?
      player = Player.new(:name => params["name"])
      if player.save
        params["team_player"]["player_id"] = player.id
      else
        params["team_player"]["player_id"] = nil
      end
    end
    team_player = TeamPlayer.new(params["team_player"])
    if team_player.save
      render :nothing => true
    else
      render :nothing => true, :status => 401
    end
  end

end
