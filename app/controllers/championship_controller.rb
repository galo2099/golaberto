require 'digest/sha1'

class ChampionshipController < ApplicationController
  N_("Championship")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def new
    @championship = Championship.new
    @categories = Category.all
  end

  def create
    @categories = Category.all
    @championship = Championship.new(championship_params)

    if @championship.save
      redirect_to :action => :show, :id => @championship
    else
      render :action => :new
    end
  end

  def list
    @championships = Championship.order("name, begin").page(params[:page])
    @name = params[:name]
    unless @name.nil?
      @championships = @championships.where("name LIKE ?", "%#{@name}%")
    end
  end

  def show
    @championship = Championship.find(params["id"])
    respond_to do |format|
      format.html {
        last_phase = @championship.phases[-1] unless @championship.phases.empty?
        redirect_to action: :phases,
                    id: @championship,
                    phase: last_phase
      }
      format.csv 
    end

  end

  def phases
    @championship = Championship.includes(:phases).find(params[:id])
    # Use an empty phase instead of nil if none is passed.
    @current_phase = Phase.new
    @current_phase = @championship.phases.includes(:teams).find(params[:phase]) if params[:phase]
    if @current_phase
      @display_odds = @current_phase.games.find_by_played(false) == nil
    end
  end

  def generate_team_json(championship, phase, group, team)
    data = []

    team_table = group.team_table do |teams, games|
      games.select{|g| g.home_id == team.id or g.away_id == team.id}.each do |g|
        teams.each_with_index do |t,idx|
          if t[0].team_id == team.id
            data << { :points => t[1].points, :position => idx + 1,
              :game => g,
              :type => g.home_score > g.away_score ?
                         g.home_id == team.id ? "w" : "l" :
                       g.home_score < g.away_score ?
                         g.away_id == team.id ? "w" : "l" :
                       "d" }
          end
        end
      end
    end

    team_table.each_with_index do |t,idx|
      # We need to change the last position to be the final position in the
      # phase instead of the position right after the team's last game
      if t[0].team_id == team.id
        data.last[:position] = idx + 1 unless data.empty?
      end
    end

    point_win = championship.point_win

    points_for_1st_place = team_table[0][1].points
    #points_for_1st_place = data.size * point_win

    chart = { options: {
                colors: [ "#0000ff", "#696969", "#ff0000", "#000000", "#0000ff", "#696969", "#ff0000", "#ffff80" ],
                grid: {
                  backgroundColor: "#FFFFFF",
                  hoverable: true,
                  clickable: true,
                  markings: [ if group.promoted > 0 then { yaxis: { from: 0.5, to: group.promoted + 0.5 },
                                                           color: "#90EE90" } end,
                              if group.relegated > 0 then { yaxis: { from: group.team_groups.size - group.relegated + 0.5, },
                                                            color: "#FFA0A0" } end ].select{|i|i},
                },
                xaxes: [
                  { ticks: (1..data.size).to_a.map{|x|[x, ""]}, },
                  { },
                ],
                yaxes: [
                  {
                    ticks: (1..group.team_groups.size).to_a.map{|x|[x, x]},
                    font: {
                      color: "#FFFFFF",
                    },
                  },
                  {
                    show: false,
                    max: points_for_1st_place,
                  },
                ]
              },
              data: [],
              tooltips: [[], []],
              urls: [[], []],
            }

    bar = [
      { data: [], yaxis: 2, bars: { show: true, barWidth: 0.8, align: "center" } },
      { data: [], yaxis: 2, bars: { show: true, barWidth: 0.8, align: "center" } },
      { data: [], yaxis: 2, bars: { show: true, barWidth: 0.8, align: "center" } },
    ]
    data.each_with_index do |d,index|
      value = [ index + 1, d[:points] ]
      case d[:type]
      when "w"
        bar[0][:data] << value
        bar[1][:data] << []
        bar[2][:data] << []
      when "d"
        bar[0][:data] << []
        bar[1][:data] << value
        bar[2][:data] << []
      when "l"
        bar[0][:data] << []
        bar[1][:data] << []
        bar[2][:data] << value
      end
    end
    chart[:data] << bar[0]
    chart[:data] << bar[1]
    chart[:data] << bar[2]

    line = { data: [],
             lines: { show: true, },
             points: { show: true, },
           }
    data.each_with_index do |d,index|
      value = [ index + 1, d[:position] ]
      chart[:tooltips][0] << sprintf(_("%s - %d points<br>%s %d x %d %s"), d[:position].ordinalize, d[:points], d[:game].home.name, d[:game].home_score, d[:game].away_score, d[:game].away.name)
      chart[:urls][0] << url_for(:controller => :game, :action => :show, :id => d[:game])
      line[:data] << value
    end
    chart[:data] << line

    return chart.to_json, team_table
  end

  def team_json
    championship = Championship.find(params["id"])
    team = Team.find(params["team"])
    phase = Phase.find(params["phase"])
    group = phase.groups.select{|g| g.teams.include? team}.first
    json, table = generate_team_json(championship, phase, group, team)

    render :text => json, :layout => false
  end

  def team
    store_location
    @championship = Championship.includes(:phases => [ :teams, { :groups => :teams }]).find(params["id"])

    # Find every team for this championship
    @teams = @championship.teams.order(:name)

    # Find the team we're looking for
    if params["team"].blank?
      params["team"] = @teams.first.id unless @teams.empty?
    end
    @team = Team.find(params["team"])

    # Find every group that this team belonged to
    @groups = @championship.phases.map{|p| p.groups}.flatten.select{|g| g.teams.include? @team}.reverse

    @group_json = []
    @groups.each_with_index do |g, idx|
      json, table = generate_team_json(@championship, g.phase, g, @team)
      @group_json << json
    end

    @played_games = @team.home_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games += @team.away_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games.sort!{|a,b| a.date <=> b.date}

    @scheduled_games = @team.home_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games += @team.away_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games.sort!{|a,b| a.date <=> b.date}

    @players = @team.team_players.where(championship_id: @championship.id).to_a
    @players.sort!{|a,b| a.player.name <=> b.player.name}.map! do |p|
      { :player => p.player,
        :team_player => p,
        :goals => p.player.goals.joins("LEFT JOIN games ON games.id = game_id").where("own_goal = ? AND games.phase_id IN (?) AND team_id = ?", false, @championship.phase_ids, @team).count,
        :penalties => p.player.goals.joins("LEFT JOIN games ON games.id = game_id").where("own_goal = ? AND penalty = ? AND games.phase_id IN (?) AND team_id = ?", false, true, @championship.phase_ids, @team).count,
        :own_goals => p.player.goals.joins("LEFT JOIN games ON games.id = game_id").where("own_goal = ? AND games.phase_id IN (?) AND team_id = ?", true, @championship.phase_ids, @team).count
      }
    end
  end

  def new_game
    @championship = Championship.find(params["id"])
    @current_phase = @championship.phases.find(params["phase"])
    @game = @current_phase.games.build
  end

  def games
    store_location
    @championship = Championship.find(params["id"])
    @current_phase = @championship.phases.find(params["phase"])
    group = params["group"]

    games = @current_phase.games
    if group.nil?
      @groups_to_show = @current_phase.groups.includes(:teams)
    else
      @groups_to_show = [ @current_phase.groups.find(group) ]
      games = @groups_to_show.first.games
    end

    @rounds = games.pluck(:round).uniq.reject{|r|r.nil?}.sort

    unless (params[:round].to_s.empty?)
      @current_round = params[:round].to_i
      games = games.where(:round => @current_round)
    end

    @games = games.order("date, round, teams.name").includes(:home, :away).page(params[:page]).references(:team)
    @total_games = games.size
  end

  def edit
    @championship = Championship.find(params["id"])
    @categories = Category.all
  end

  def update
    @championship = Championship.find(params["id"])
    @categories = Category.all

    @championship.attributes = championship_params

    saved = @championship.save
    new_empty = false

    @phase = @championship.phases.build(phase_params)
    new_empty = @phase.name.empty?

    saved = @phase.save and saved unless new_empty

    if saved and new_empty
      redirect_to :action => "show", :id => @championship
    else
      render :action => "edit"
    end
  end

  def crowd
    store_location
    @championship = Championship.find(params["id"])

    @average = @championship.games.group(:home).average(:attendance).sort{|a,b| b[1].to_i <=> a[1].to_i}
    @maximum = @championship.games.group(:home).maximum(:attendance)
    @minimum = @championship.games.group(:home).minimum(:attendance)
    @count = @championship.games.group(:home).count(:attendance)
    @games = @championship.games.order("attendance DESC").page(params[:page]).per_page(10)
  end

  def destroy
    Championship.find(params["id"]).destroy
    redirect_to action: :list
  end

  def top_goalscorers
    @championship = Championship.find(params["id"])
    # we do pagination by hand because will_paginate doesn't like the
    # OrderedHash returned by the count method
    page = (params[:page] || 1).to_i
    per_page = 30
    offset = (page - 1) * per_page
    scorers = @championship.goals.group(:player).where(:own_goal => false).order("count_all DESC").count
    @scorer_pagination = WillPaginate::Collection.new(page, per_page, scorers.size)
    @scorers = scorers.keys.sort{|a,b|scorers[b] <=> scorers[a]}[offset, per_page].map{|k| [ k, scorers[k] ] }
    players = @scorers.map{|p,c| p}
    @teams = @championship.goals.group(:player_id, :team_id).where(:own_goal => false).count.inject(Hash.new) do |h,t|
      player = Player.find(t[0][0])
      team = Team.find(t[0][1])
      h[player] = [ team ] + h[player].to_a
      h
    end
    @own = @championship.goals.group(:player).where(:own_goal => true, :player_id => players).count
    @penalty = @championship.goals.group(:player).where(:penalty => true, :player_id => players).count
  end

  private
  def championship_params
    params.require(:championship).permit(:name, :begin, :end, :point_win, :point_draw, :point_loss, :category_id, :show_country)
  end

  def phase_params
    params.require(:phase).permit(:name, :order_by)
  end
end
