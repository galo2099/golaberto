class ChampionshipController < ApplicationController
  helper :phase

  def index
    redirect_to :action => :list
  end

  def new
    @championship = Championship.new
  end

  def create
    @championship = Championship.new(@params["championship"])

    if @championship.save
      redirect_to :action => :show, :id => @championship
    else
      render :action => :new
    end
  end

  def list
    @total = Championship.count
    @championship_pages, @championships = paginate :championships, :order => "begin DESC, name"
  end

  def show
    championship = Championship.find(@params["id"])
    if championship.phases.empty?
      render :text => "Championship has no phases", :layout => true
    else
      redirect_to :action => 'phases',
                  :id => championship,
                  :phase => championship.phases[-1]
    end
  end

  def phases
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
    @all_games = @current_phase.games.find(:all,
                                           :order => "date")
    @class = Hash.new
    @current_phase.groups.each do |group|
      group.teams.each do |team|
        @class[team.id] = ChampionshipHelper::TeamCampaign.new team, @all_games
      end
    end
  end

  def team
    @championship = Championship.find(@params["id"])
    @teams = @championship.phases.map{|p| p.groups}.flatten.map{|g| g.teams}.flatten.sort{|a,b| a.name <=> b.name}.uniq
    if @params["team"].to_s == ""
      @params["team"] = @teams.first.id
    end
    @team = Team.find(@params["team"])
    @played_games = @team.home_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, 'played')
    @played_games += @team.away_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, 'played')
    @played_games.sort!{|a,b| a.date <=> b.date}

    @scheduled_games = @team.home_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, 'scheduled')
    @scheduled_games += @team.away_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, 'scheduled')
    @scheduled_games.sort!{|a,b| a.date <=> b.date}
  end

  def new_game
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
    @game = @current_phase.games.build
  end

  def games
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])

    items_per_page = 30
    @games_pages, @games = paginate :games, :order => "date", :per_page => items_per_page, :conditions => "games.phase_id = #{@current_phase.id}", :include => [ "home", "away" ]

    if request.xml_http_request?
      render :partial => "game_table", :layout => false
    else
      phases
    end
  end

  def edit
    @championship = Championship.find(@params["id"])
  end

  def update
    @championship = Championship.find(@params["id"])
    @championship.attributes = @params["championship"]

    saved = @championship.save
    new_empty = false

    @phase = @championship.phases.build(@params["phase"])
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
