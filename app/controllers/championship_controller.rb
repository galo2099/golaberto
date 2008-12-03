class ChampionshipController < ApplicationController
  N_("Championship")

  before_filter :login_required, :except => [ :index, :list, :show, :phases,
                                              :crowd, :team, :games, :team_json,
                                              :top_goalscorers ]

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
    @championships = Championship.paginate :order => "name, begin", :page => params[:page]
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
    if @current_phase
      @display_odds = @current_phase.games.find_by_played(:first, false) == nil
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

    #points_for_1st_place = team_table[0][1].points
    points_for_1st_place = data.size * point_win


    chart = Chart.new(sprintf(_("%s Campaign"), team.name))
    chart.bg_colour = "#FFFFFF"
    tooltip = Tooltip.new
    tooltip.set_hover
    chart.tooltip = tooltip
    chart.x_axis = XAxis.new do |x|
      x.labels = [ "" ]
      x.offset = true
    end
    chart.y_axis = YAxis.new do |y|
      y.title = "Position"
      y.labels = (1..group.team_groups.size).to_a.reverse.map{|i| i.to_s}
      y.offset = true
    end

    promotion_zone = Shape.new("#00FF00")
    promotion_zone.append_value ShapePoint.new(-0.5, group.team_groups.size - 0.5)
    promotion_zone.append_value ShapePoint.new(data.size - 0.5, group.team_groups.size - 0.5)
    promotion_zone.append_value ShapePoint.new(data.size - 0.5, group.team_groups.size - group.promoted - 0.5)
    promotion_zone.append_value ShapePoint.new(-0.5, group.team_groups.size - group.promoted - 0.5)
    promotion_zone.alpha = 0.4
    chart.add_element promotion_zone

    relegation_zone = Shape.new("#FF0000")
    relegation_zone.append_value ShapePoint.new(-0.5, -0.5)
    relegation_zone.append_value ShapePoint.new(- 0.5, group.relegated - 0.5)
    relegation_zone.append_value ShapePoint.new(data.size - 0.5, group.relegated - 0.5)
    relegation_zone.append_value ShapePoint.new(data.size - 0.5, -0.5)
    relegation_zone.alpha = 0.4
    chart.add_element relegation_zone

    bar = BarGlass.new
    data.each do |d|
      value = BarValue.new(d[:points].to_f/(points_for_1st_place) * group.team_groups.size - 0.5, -0.5, {
        :on_click => url_for(:controller => :game, :action => :show, :id => d[:game]),
        :tip => _("#{d[:position].ordinalize} - #{d[:points]} points<br>#{d[:game].home.name} #{d[:game].home_score} x #{d[:game].away_score} #{d[:game].away.name}"),
        :colour => case d[:type]
                     when "w"
                       "0000ff"
                     when "d"
                       "808080"
                     when "l"
                       "ff0000"
                     end })
      bar.append_value value
    end
    chart.add_element bar
    line = LineHollow.new
    line.values = []
    line.colour = "#000000"
    line.dot_size = 4
    line.halo_size = 1
    data.each do |d|
      value = DotValue.new(group.team_groups.size - d[:position], {
        :tip => _("#{d[:position].ordinalize} - #{d[:points]} points<br>#{d[:game].home.name} #{d[:game].home_score} x #{d[:game].away_score} #{d[:game].away.name}"),
        :on_click => url_for(:controller => :game, :action => :show, :id => d[:game])})
      line.append_value value
    end
    chart.add_element line

    return chart.render, team_table
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
    @championship = Championship.find(params["id"])

    # Find every team for this championship
    @teams = @championship.phases.map{|p| p.groups}.flatten.map{|g| g.teams}.flatten.sort{|a,b| a.name <=> b.name}.uniq

    # Find the team we're looking for
    if params["team"].to_s == ""
      params["team"] = @teams.first.id
    end
    @team = Team.find(params["team"])

    # Find every group that this team belonged to
    @groups = @championship.phases.map{|p| p.groups}.flatten.select{|g| g.teams.include? @team}.reverse

    @group_json = []
    @group_table = []
    @graph = open_flash_chart_object(550, 300, { :div_name => "campaign_graph", :function => "load_graph_data0" })
    @groups.each_with_index do |g, idx|
      json, table = generate_team_json(@championship, g.phase, g, @team)
      @group_json << json
      @group_table << table
    end

    @played_games = @team.home_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, true)
    @played_games += @team.away_games.find_all_by_phase_id_and_played(
        @championship.phase_ids, true)
    @played_games.sort!{|a,b| a.date <=> b.date}

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
    @championship = Championship.find(params["id"])
    @current_phase = @championship.phases.find(params["phase"])
    @game = @current_phase.games.build
  end

  def games
    store_location
    @championship = Championship.find(params["id"])
    @current_phase = @championship.phases.find(params["phase"])
    @group = params["group"]

    conditions = nil
    if @group.nil?
      @groups_to_show = @current_phase.groups
    else
      @groups_to_show = [ @current_phase.groups.find(@group) ]
      conditions ||= {}
      teams = @groups_to_show.first.teams.map{|t| t.id}
      conditions.merge!({ :home_id => teams })
    end

    @rounds = @current_phase.games.find(:all, :group => :round, :order => :round, :conditions => conditions).map{|g| g.round }.compact

    unless (params[:round].to_s.empty?)
      @current_round = params[:round].to_i
      conditions ||= {}
      conditions.merge!({ :round => @current_round })
    end

    @games = @current_phase.games.paginate :order => "date, round, time, teams.name", :conditions => conditions, :include => [ "home", "away" ], :page => params[:page]

    phases
  end

  def edit
    @championship = Championship.find(params["id"])
    @categories = Category.find(:all)
  end

  def update
    @championship = Championship.find(params["id"])
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

  def crowd
    store_location
    @championship = Championship.find(params["id"])

    @average = @championship.games.average(:attendance, :group => :home, :order => "avg_attendance DESC")
    @maximum = @championship.games.maximum(:attendance, :group => :home)
    @minimum = @championship.games.minimum(:attendance, :group => :home)
    @count = @championship.games.count(:attendance, :group => :home)
    @games = @championship.games.paginate(:order => "attendance DESC", :page => params[:page], :per_page => 10)
  end

  def destroy
    Championship.find(params["id"]).destroy
    redirect_to :action => "list"
  end

  def top_goalscorers
    @championship = Championship.find(params["id"])
    @goals = @championship.goals.paginate :finder => :count,
                                          :page => params[:page],
                                          :group => :player,
                                          :conditions => { :own_goal => 0 },
                                          :limit => 10,
                                          :order => "count_all DESC"
    players = @goals.map{|p,c| p}
    @teams = @championship.goals.calculate :group_concat,
                                           :all,
                                           :group => :player,
                                           :conditions => { :player_id => players },
                                           :select => "DISTINCT(team_id)"
    @teams.map! do |p,t|
      [p, t.split(',').map{ |id| Team.find id } ]
    end
    @own = @championship.goals.count :group => :player,
                                     :conditions => { :own_goal => 1,
                                                      :player_id => players }
    @penalty = @championship.goals.count :group => :player,
                                         :conditions => { :penalty => 1,
                                                          :player_id => players }
  end
end
