class ChampionshipController < ApplicationController
  scaffold :championship
  helper :phase

  def show
    championship = Championship.find(@params["id"])
    redirect_to :action => 'phases',
                :id => championship,
                :phase => championship.phases[-1]
  end

  def phases
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
      games = @current_phase.games.find(:all, :conditions => "played = 'played'")
      @class = Hash.new
      @current_phase.teams.each do |team|
        @class[team.id] = ChampionshipHelper::TeamCampaign.new team, games
      end
  end

  def games
    items_per_page = 30
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
    games = @current_phase.games.find(:all)
    @class = Hash.new
    @current_phase.teams.each do |team|
      @class[team.id] = ChampionshipHelper::TeamCampaign.new team, games
    end

    @total = games.size
    @games_pages, @games = paginate :games, :order => "date", :per_page => items_per_page, :conditions => "games.phase_id = #{@current_phase.id}"

    if request.xml_http_request?
      render :partial => "game_list", :layout => false
    end
  end

  def edit
    @championship = Championship.find(@params["id"])
  end

  def update
    @championship = Championship.find(@params["championship"]["id"])
    @championship.attributes = @params["championship"]

    saved = @championship.save
    new_empty = false

    @params["phases"].each { |key, value|
      if key == "new"
        phase = @championship.build_to_phases(value)
        new_empty = true if value["name"].empty?
      else
        phase = Phase.find(key)
        phase.attributes = value
      end

      unless phase.name.empty?
        saved = false unless phase.save
      end
    }

    if saved and new_empty
      redirect_to :action => "show", :id => @championship.id
    else
      @target = "update"
      render "championship/edit" 
    end
  end

  def destroy
    Championship.find(@params["id"]).destroy
    redirect_to :action => "list" 
  end

end
