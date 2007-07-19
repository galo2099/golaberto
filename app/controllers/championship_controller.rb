class ChampionshipController < ApplicationController
  before_filter :login_required, :only => [ :new, :create, :new_game,
                                            :edit, :update, :destroy ]

  def index
    redirect_to :action => :list
  end

  def new
    @championship = Championship.new
  end

  def create
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
    if @championship.phases.empty?
      render :partial => "nav_side", :layout => true
    else
      redirect_to :action => 'phases',
                  :id => @championship,
                  :phase => @championship.phases[-1]
    end
  end

  def sort_teams(phase, group, team_class)
    columns = phase.sort.split(/,\s*/)
    sorter = lambda { |b,a|
      ret = 0
      columns.detect do |column|
        case column
        when "pt"
          ret = team_class[a.id].points <=> team_class[b.id].points
        when "w"
          ret = team_class[a.id].wins <=> team_class[b.id].wins
        when  "gd"
          ret = team_class[a.id].goals_diff <=> team_class[b.id].goals_diff
        when "gf"
          ret = team_class[a.id].goals_for <=> team_class[b.id].goals_for
        when "name"
          ret = a.name <=> b.name
        when "g_average"
          ret = team_class[a.id].goals_avg <=> team_class[b.id].goals_avg
        when "gp"
          ret = team_class[a.id].goals_pen <=> team_class[b.id].goals_pen
        when "g_away"
          ret = team_class[a.id].goals_away <=> team_class[b.id].goals_away
        when "bias"
          ret = group.team_groups.find(
              :first,
              :conditions => [ "team_id = ?", a.id ]).bias <=>
                group.team_groups.find(:first,
              :conditions => [ "team_id = ?", b.id ]).bias
        end
        ret != 0
      end
      ret
    }
    group.team_groups.sort do |a,b|
      sorter.call a.team, b.team
    end.map do |t|
      [ t, team_class[t.team.id] ]
    end
  end

  private :sort_teams

  def phases
    @championship = Championship.find(@params["id"])
    @current_phase = @championship.phases.find(@params["phase"])
    @all_games = @current_phase.games.find(:all,
                                           :order => "date")
    stats = Hash.new
    @current_phase.groups.each do |group|
      group.team_groups.each do |team_group|
        stats[team_group.team.id] = ChampionshipHelper::TeamCampaign.new(team_group, @all_games)
      end
    end

    @sorted_teams = @current_phase.groups.map do |group|
      sort_teams(@current_phase, group, stats)
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
        color = earned == @championship.point_win ? "blue" : earned == @championship.point_draw ? "grey" : "red"
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
          :conditions => [ "own_goal = 0 AND games.phase_id IN (?)", @championship.phase_ids]),
        :penalties => p.player.goals.count(
          :joins => "LEFT JOIN games ON games.id = game_id",
          :conditions => [ "own_goal = 0 AND penalty = 1 AND games.phase_id IN (?)", @championship.phase_ids]),
        :own_goals => p.player.goals.count(
          :joins => "LEFT JOIN games ON games.id = game_id",
          :conditions => [ "own_goal = 1 AND games.phase_id IN (?)", @championship.phase_ids])
      }
    end
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
