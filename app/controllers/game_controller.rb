class GameController < ApplicationController

  def show
    @game = Game.find(@params["id"])
  end

  def destroy
    game = Game.find(@params["id"])
    options = { :controller => :championship, :action => :games, :id => game.phase.championship, :phase => game.phase }
    game.destroy
    redirect_to options
  end

  def edit
    begin
      @game = Game.find(@params["id"])
      @referees = Referee.find(:all, :order => :name)
      @stadiums = Stadium.find(:all, :order => :name)
      @home_players = @game.home.team_players.find(
          :all,
          :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
      @away_players = @game.away.team_players.find(
          :all,
          :conditions => [ "championship_id = ?", @game.phase.championship ]).map {|p| p.player}
    rescue
      flash[:notice] = "Game not found"
      redirect_to :action => 'list'
    end
  end

  def create
    @championship = Championship.find(@params["championship"])
    @phase = Phase.find(@params["phase"])
    @game = @phase.games.build(@params["game"])
    @game.date = DateTime.now
    if @game.save
      redirect_to :action => :edit, :id => @game
    else
      flash[:notice] = @game.errors.full_messages
      redirect_to :controller => :championship, :action => :new_game, :id => @championship, :phase => @phase
    end
  end

  def update
    @game = Game.find(@params["id"])

    date = @params["game"].delete("date")
    hour = @params["date"]["hour"]
    minute = @params["date"]["minute"]
    @game.attributes = @params["game"]
    unless date.empty?
      @game.date = DateTime.strptime("#{date} - #{hour}:#{minute}",
                                     ActiveSupport::CoreExtensions::Date::Conversions::DATE_FORMATS[:date] + " - " + ActiveSupport::CoreExtensions::Date::Conversions::DATE_FORMATS[:time])
    end

    saved = @game.save

    if saved
      flash[:notice] = "Game saved"
      if (@params["redirect"])
        redirect_to @params["redirect"]
      else
        redirect_to :action => :show, :id => @game
      end
    else
      @referees = Referee.find(:all, :order => :name)
      @stadiums = Stadium.find(:all, :order => :name)
      render :action => :edit
    end
  end
end
