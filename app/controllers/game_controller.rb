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

  def prepare_for_edit
    @referees = Referee.find(:all, :order => :name)
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
                                      "%d/%m/%Y - %H:%M")
    end

    saved = @game.save

    @game.home_goals.clear
    @game.away_goals.clear

    @goals = Array.new
    @game.home_score.times do |i|
      goal = @game.home_goals.build(@params["home_goal"][i.to_s])
      if goal.player
        goal.team_id = goal.own_goal == "0" ? @game.home_id : @game.away_id
        saved = goal.save and saved
        @goals.push goal
      end
    end
    @game.away_score.times do |i|
      goal = @game.away_goals.build(@params["away_goal"][i.to_s])
      if goal.player
        goal.team_id = goal.own_goal == "0" ? @game.away_id : @game.home_id
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
end
