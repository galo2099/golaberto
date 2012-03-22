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
    @championship = Championship.new(params["championship"])

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

    #points_for_1st_place = team_table[0][1].points
    points_for_1st_place = data.size * point_win

    chart = { :title => { :text => sprintf(_("%s Campaign"), team.name) },
              :bg_colour => "#FFFFFF",
              :tooltip => { :mouse => 2 },
              :x_axis => {
                  :labels => { :labels => [ "" ] },
                  :offset => true,
              },
              :y_axis => {
                  :title => "Position",
                  :labels => (1..group.team_groups.size).to_a.reverse.map{|i| i.to_s},
                  :offset => true,
              },
              :elements => [] }

    promotion_zone = { :type => "shape",
                       :colour => "#00FF00",
                       :alpha => 0.4,
                       :values => [] }
    promotion_zone[:values] << { :x => -0.5, :y => group.team_groups.size - 0.5 }
    promotion_zone[:values] << { :x => data.size - 0.5, :y => group.team_groups.size - 0.5 }
    promotion_zone[:values] << { :x => data.size - 0.5, :y => group.team_groups.size - group.promoted - 0.5 }
    promotion_zone[:values] << { :x => -0.5, :y => group.team_groups.size - group.promoted - 0.5 }
    chart[:elements] << promotion_zone

    relegation_zone = { :type => "shape",
                        :colour => "#FF0000",
                        :alpha => 0.4,
                        :values => [] }
    relegation_zone[:values] << { :x => -0.5, :y => -0.5 }
    relegation_zone[:values] << { :x => - 0.5, :y => group.relegated - 0.5 }
    relegation_zone[:values] << { :x => data.size - 0.5, :y => group.relegated - 0.5 }
    relegation_zone[:values] << { :x => data.size - 0.5, :y => -0.5 }
    chart[:elements] << relegation_zone

    bar = { :type => "bar_glass",
            :values => [] }
    data.each do |d|
      value = { :top => d[:points].to_f/(points_for_1st_place) * group.team_groups.size - 0.5,
                :bottom => -0.5,
                "on-click" => url_for(:controller => :game, :action => :show, :id => d[:game]),
                :tip => sprintf(_("%s - %d points<br>%s %d x %d %s"), d[:position].ordinalize, d[:points], d[:game].home.name, d[:game].home_score, d[:game].away_score, d[:game].away.name),
                :colour => case d[:type]
                    when "w"
                    "0000ff"
                    when "d"
                    "808080"
                    when "l"
                    "ff0000"
                    end }
      bar[:values] << value
    end
    chart[:elements] << bar

    line = { :type => "line_hollow",
             :values => [],
             :colour => "#000000",
             "dot-size" => 4,
             "halo-size" => 1 }
    data.each do |d|
      value = { :value => group.team_groups.size - d[:position],
                :tip => sprintf(_("%s - %d points<br>%s %d x %d %s"), d[:position].ordinalize, d[:points], d[:game].home.name, d[:game].home_score, d[:game].away_score, d[:game].away.name),
                "on-click" => url_for(:controller => :game, :action => :show, :id => d[:game]) }
      line[:values] << value
    end
    chart[:elements] << line

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
    @teams = @championship.phases.map{|p| p.teams}.flatten.sort{|a,b| a.name <=> b.name}.uniq

    # Find the team we're looking for
    if params["team"].to_s == ""
      params["team"] = @teams.first.id
    end
    @team = Team.find(params["team"])

    # Find every group that this team belonged to
    @groups = @championship.phases.map{|p| p.groups}.flatten.select{|g| g.teams.include? @team}.reverse

    @group_json = []
    @graph = open_flash_chart_object(550, 300, { :div_name => "campaign_graph", :function => "load_graph_data0" }).html_safe
    @groups.each_with_index do |g, idx|
      json, table = generate_team_json(@championship, g.phase, g, @team)
      @group_json << json
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

    games = @current_phase.games.scoped
    if group.nil?
      @groups_to_show = @current_phase.groups.includes(:teams).includes(:teams).all
    else
      @groups_to_show = [ @current_phase.groups.find(group) ]
      games = @groups_to_show.first.games
    end

    @rounds = games.pluck(:round).uniq.reject{|r|r.nil?}.sort

    unless (params[:round].to_s.empty?)
      @current_round = params[:round].to_i
      games = games.where(:round => @current_round)
    end

    @games = games.paginate :order => "date, round, time, teams.name", :include => [ "home", "away" ], :page => params[:page]
  end

  def edit
    @championship = Championship.find(params["id"])
    @categories = Category.all
  end

  def update
    @championship = Championship.find(params["id"])
    @categories = Category.all

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

    @average = @championship.games.group(:home).average(:attendance).sort{|a,b| b[1].to_i <=> a[1].to_i}
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
    # we do pagination by hand because will_paginate doesn't like the
    # OrderedHash returned by the count method
    page = (params[:page] || 1).to_i
    per_page = 30
    offset = (page - 1) * per_page
    scorers = @championship.goals.group(:player).where(:own_goal => false).order("count_all DESC").count
    @scorer_pagination = WillPaginate::Collection.new(page, per_page, scorers.size)
    @scorers = scorers.keys[offset, per_page].map{|k| [ k, scorers[k] ] }
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

  def open_flash_chart_object(width, height, options = {})
    protocol = options[:protocol] || "http"
    base = options[:base] || "/"
    function = options[:function]
    url = options[:url] || ""
    url = CGI::escape(url)
    # need something that will not be repeated on the same request
    special_hash = Base64.encode64(Digest::SHA1.digest("#{rand(1<<64)}/#{Time.now.to_f}/#{Process.pid}/#{url}"))[0..7]
    # only good characters for our div
    special_hash = special_hash.gsub(/[^a-zA-Z0-9]/,rand(10).to_s)
    obj_id   = "chart_#{special_hash}"  # some sequencing without all the work of tracking it
    div_name = options[:div_name] || "flash_content_#{special_hash}"

    # NOTE: users should put this in the <head> section themselves:
    ## <script type="text/javascript" src="#{base}/javascripts/swfobject.js"></script>

    <<-HTML
    <div style="min-height: #{height}px"><div id="#{div_name}"></div></div>
    <script type="text/javascript">
    swfobject.embedSWF("#{base}open-flash-chart.swf", "#{div_name}", "#{width}", "#{height}", "9.0.0", "expressInstall.swf", #{ url.empty? ? "" : "{'data-file':'#{url}'}, "}#{ function ? "{'get-data':'#{function}'}, " : "" }{"wmode":"transparent"});
    </script>
    <noscript>
      <object classid="clsid:d27cdb6e-ae6d-11cf-96b8-444553540000" codebase="#{protocol}://fpdownload.macromedia.com/pub/shockwave/cabs/flash/swflash.cab#version=8,0,0,0" width="#{width}" height="#{height}" id="#{obj_id}" align="middle" wmode="transparent">
        <param name="allowScriptAccess" value="sameDomain" />
        <param name="movie" value="#{base}open-flash-chart.swf?data=#{url}" />
        <param name="quality" value="high" />
        <param name="bgcolor" value="#FFFFFF" />
        <embed src="#{base}open-flash-chart.swf?data=#{url}" quality="high" bgcolor="#FFFFFF" width="#{width}" height="#{height}" name="#{obj_id}" align="middle"  allowScriptAccess="sameDomain" type="application/x-shockwave-flash" pluginspage="#{protocol}://www.macromedia.com/go/getflashplayer" id="#{obj_id}" />
      </object>
    </noscript>
    HTML
  end
end
