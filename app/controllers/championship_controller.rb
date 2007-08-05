class ChampionshipController < ApplicationController
  before_filter :login_required, :except => [ :index, :list, :show, :phases,
                                              :team, :games ] 

  def index
    redirect_to :action => :list
  end

  def new
    @championship = Championship.new
    @categories = Category.find(:all)
  end

  def create
    @categories = Category.find(:all)
    @championship = Championship.new(params["championship"])
    @championship.begin = Date.strptime(params["championship"]["begin"],
                                        "%d/%m/%Y")
    @championship.end = Date.strptime(params["championship"]["end"],
                                      "%d/%m/%Y")

    if @championship.save
      redirect_to :action => :show, :id => @championship
    else
      render :action => :new
    end
  end

  def list
    @total = Championship.count
    @championship_pages, @championships = paginate :championships, :order => "name, begin"
  end

  def show
    @championship = Championship.find(params["id"])
    last_phase = @championship.phases[-1] unless @championship.phases.empty?
    redirect_to :action => 'phases',
                :id => @championship,
                :phase => last_phase
  end

  def phases
    @championship = Championship.find(params[:id])
    @current_phase = @championship.phases.find(params[:phase]) if params[:phase]
  end

  def team
    store_location
    @championship = Championship.find(@params["id"])

    # Find every team for this championship
    @teams = @championship.phases.map{|p| p.groups}.flatten.map{|g| g.teams}.flatten.sort{|a,b| a.name <=> b.name}.uniq

    # Find the team we're looking for
    if @params["team"].to_s == ""
      @params["team"] = @teams.first.id
    end
    @team = Team.find(@params["team"])

    # Find every group that this team belonged to
    @groups = @championship.phases.map{|p| p.groups}.flatten.select{|g| g.teams.include? @team}.reverse

    @played_games = @team.home_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, true)
    @played_games += @team.away_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, true)
    @played_games.sort!{|a,b| a.date <=> b.date}

    games = 0
    @data_for_graph = Array.new
    @championship.phases.each do |phase|
      points = 0
      data = @played_games.select{|g| g.phase.id == phase.id}.map do |game|
        earned = @championship.point_loss
        if game.home_score > game.away_score
          earned = @championship.point_win if game.home == @team
        elsif game.home_score < game.away_score
          earned = @championship.point_win if game.away == @team
        else
          earned = @championship.point_draw
        end
        points += earned
        games += 1
        color = earned == @championship.point_win ? "blue" : earned == @championship.point_draw ? "gray" : "red"
        game_str = game.formatted_date + " "
        game_str << game.home.name + " " + game.home_score.to_s + " x "
        game_str << game.away_score.to_s + " " + game.away.name
        game_str = "header=[#{points} points] body=[#{game_str}] cssbody=[popupbody]"
        { :title => game_str,
          :color => color,
          :value => points,
          :game => game }
      end
      @data_for_graph.push data
    end

    @scheduled_games = @team.home_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, false, :include => [ :home, :away ])
    @scheduled_games += @team.away_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, false, :include => [ :home, :away ])
    @scheduled_games.sort!{|a,b| a.date <=> b.date}

    @players = @team.team_players.find_all_by_championship_id(@championship.id)
    @players.sort!{|a,b| a.player.name <=> b.player.name}.map! do |p|
      { :player => p.player,
        :team_player => p,
        :goals => p.player.goals.count(
          :joins => "LEFT JOIN games ON games.id = game_id",
          :conditions => [ "own_goal = 0 AND games.phase_id IN (?) AND team_id = ?", @championship.phase_ids, @team]),
        :penalties => p.player.goals.count(
          :joins => "LEFT JOIN games ON games.id = game_id",
          :conditions => [ "own_goal = 0 AND penalty = 1 AND games.phase_id IN (?) AND team_id = ?", @championship.phase_ids, @team]),
        :own_goals => p.player.goals.count(
          :joins => "LEFT JOIN games ON games.id = game_id",
          :conditions => [ "own_goal = 1 AND games.phase_id IN (?) AND team_id = ?", @championship.phase_ids, @team])
      }
    end
  end

  def new_game
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
    @game = @current_phase.games.build
  end

  def games
    store_location
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])

    items_per_page = 30
    @games_pages, @games = paginate :games, :order => "date, round, time, teams.name", :per_page => items_per_page, :conditions => "games.phase_id = #{@current_phase.id}", :include => [ "home", "away" ]

    if request.xml_http_request?
      render :partial => "game_table", :layout => false
    else
      phases
    end
  end

  def edit
    @championship = Championship.find(@params["id"])
    @categories = Category.find(:all)
  end

  def update
    @championship = Championship.find(@params["id"])
    @categories = Category.find(:all)

    begin_date = params[:championship].delete(:begin)
    @championship.begin = Date.strptime(begin_date, "%d/%m/%Y") unless begin_date.empty?
    end_date = params[:championship].delete(:end)
    @championship.end = Date.strptime(end_date, "%d/%m/%Y") unless end_date.empty?
    @championship.attributes = params[:championship]

    saved = @championship.save
    new_empty = false

    @phase = @championship.phases.build(params[:phase])
    new_empty = @phase.name.empty?

    saved = @phase.save and saved unless new_empty

    if saved and new_empty
      redirect_to :action => "show", :id => @championship
    else
      render :action => "edit" 
    end
  end

  def destroy
    Championship.find(@params["id"]).destroy
    redirect_to :action => "list" 
  end

end
