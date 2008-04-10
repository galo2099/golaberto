class ChampionshipController < ApplicationController
  before_filter :login_required, :except => [ :index, :list, :show, :phases,
                                              :crowd, :team, :games, :team_xml,
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
  end

  def team_xml(championship, phase, group, team)
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

    buffer = ""
    xml = Builder::XmlMarkup.new(:target => buffer)
    xml.instruct! :xml, :version => "1.0", :encoding => "UTF8"
    xml.graph :PYAxisName => "Position",
              :showHoverCap => 1,
              :shownames => 0,
              :SYAxisMaxValue => data.size * point_win,
              :PYAxisMinValue => - group.team_groups.size,
              :PYAxisMaxValue => - 1,
              :showDivLineSecondaryValue => 0,
              :showSecondaryLimits => 0,
              :showLegend => 0,
              :plotGradientcolor => "",
              :yAxisValuesStep => group.team_groups.size / 23 + 1,
              :caption => "#{team.name} Campaign",
              :NumDivLines => group.team_groups.size - 2,
              :adjustDiv => 0,
              :decimalPrecision => 2 do |x|
      x.categories do |x|
        data.each_with_index do |d, idx|
          x.category :name => (idx + 1).to_s
        end
      end
      x.dataset :seriesname => "Points", :renderAs => "column", :parentYAxis => "S", :showValues => 0 do |x|
        data.each_with_index do |d, idx|
          x.set :value => d[:points],
            :link => url_for(:controller => :game, :action => :show, :id => d[:game]),
            :toolText => "#{d[:position].ordinalize} - #{d[:points]} points\\n#{d[:game].home.name} #{d[:game].home_score} x #{d[:game].away_score} #{d[:game].away.name}",
            :color => case d[:type]
                      when "w"
                        "0000ff"
                      when "d"
                        "808080"
                      when "l"
                        "ff0000"
                      end
        end
      end
      x.dataset :seriesname => "Position", :renderAs => "line", :parentYAxis => "P", :color => "000000", :showValues => 0 do |x|
        data.each_with_index do |d, idx|
          x.set :value => - d[:position],
            :toolText => "#{d[:position].ordinalize} - #{d[:points]} points\\n#{d[:game].home.name} #{d[:game].home_score} x #{d[:game].away_score} #{d[:game].away.name}",
            :link => url_for(:controller => :game, :action => :show, :id => d[:game])
        end
      end
      x.trendlines do |x|
        x.line :parentYAxis => "P", :startValue => - (group.promoted + 0.5), :endValue => -1, :color => "00ff00", :displayValue => " ", :isTrendZone => 1, :showOnTop => 1, :alpha => 20
      end if group.promoted > 0
      x.trendlines do |x|
        x.line :parentYAxis => "P", :startValue => - (group.team_groups.size - group.relegated + 0.5), :endValue => -group.team_groups.size, :color => "ff0000", :displayValue => " ", :isTrendZone => 1, :showOnTop => 1, :valueOnRight => 1, :alpha => 20
      end if group.relegated > 0
    end
    buffer.gsub!("'", "&rsquo;")
    buffer.gsub!('"', "'")
    return buffer, team_table
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

    @group_xml = []
    @group_table = []
    @groups.each do |g|
      xml, table = team_xml(@championship, g.phase, g, @team)
      @group_xml << xml
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

    conditions = { :phase_id => @current_phase }

    @rounds = @current_phase.games.find(:all, :group => :round, :order => :round).map{|g| g.round }.compact

    unless (params[:round].to_s.empty?)
      @current_round = params[:round].to_i
      conditions.merge!({ :round => @current_round })
    end

    items_per_page = 30
    @games_pages, @games = paginate :games, :order => "date, round, time, teams.name", :per_page => items_per_page, :conditions => conditions, :include => [ "home", "away" ]

    if request.xml_http_request?
      render :partial => "game_table", :layout => false
    else
      phases
    end
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
