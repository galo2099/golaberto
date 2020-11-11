require 'digest/sha1'

class ChampionshipController < ApplicationController
  include ApplicationHelper

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
    @championships = Championship.order("region, region_name, name, begin")
    @name = params[:name]
    unless @name.blank?
      @championships = @championships.where("name LIKE ?", "%#{@name}%")
    end
    @categories = Category.all
    @category = params[:category] || 1
    unless @category.nil?
      @championships = @championships.where(category: @category)
    end

    countries_with_championships = { @country => 0 }
    countries_with_championships.merge!(@championships.where(region: Championship.regions["national"]).group(:region_name).size)
    countries_found = []
    countries_not_found = []
    golaberto_options_for_country_select.each do |translated_country, original_country|
      count = countries_with_championships[original_country]
      unless @continent.blank?
        unless Continent::ALL[@continent].countries.map{|c|c.name}.include? original_country
          next
        end
      end
      if count.nil? then
        countries_not_found << [translated_country, original_country]
      else
        countries_found << [translated_country + " (#{count})", original_country]
      end
    end

    @country_list = [[s_("Country|All") + " (#{@championships.where(region: Championship.regions["national"]).size})", ""]] + countries_found + countries_not_found
    @countries = {}
    @countries[""] = @country_list
    ApplicationHelper::Continent::ALL.each do |name, c|
      @countries[name] = [[s_("Country|All") + " (#{@championships.where(region_name: c.countries.map{|c|c.name}).size})", ""]] + @country_list.select{|_, n| ApplicationHelper::Continent.country_to_continent[n] == c}
    end

    @region = params[:region]
    unless @region.blank?
      @championships = @championships.where(region: Championship.regions[@region])
    end
    @country_name = params[:country_name] || ""
    @continent_name = params[:continent_name] || ""
    @continent = ApplicationHelper::Continent::ALL[@continent_name]
    if @continent
      @country_name = nil unless @continent.countries.map{|c|c.name}.include? @country_name
    end
    if (@region == "national" && !@country_name.blank?) then
      @championships = @championships.where(region_name: @country_name)
    elsif (@region == "national" && @continent) then
      @championships = @championships.where(region_name: @continent.countries.map{|c|c.name})
    elsif @region == "continental" && !@continent_name.blank? then
      @championships = @championships.where(region_name: @continent_name)
    end

    @championships = @championships.page(params[:page])
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

    points_for_1st_place = team_table[0][1].points

    chart = { options: {
                colors: [ "#0000ff", "#696969", "#ff0000", "#000000", "#0000ff", "#696969", "#ff0000", "#ffff80" ],
                grid: {
                  backgroundColor: "#FFFFFF",
                  hoverable: true,
                  clickable: true,
                  markings: group.zones.map{|z| z["position"].map{|p| { yaxis: { from: p-0.5, to: p+0.5 }, color: z["color"] } } }.flatten,
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
    json, _ = generate_team_json(championship, phase, group, team)

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
      json, _ = generate_team_json(@championship, g.phase, g, @team)
      @group_json << json
    end

    @played_games = @team.home_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games += @team.away_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games.sort!{|a,b| a.date <=> b.date}

    @scheduled_games = @team.home_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games += @team.away_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games.sort!{|a,b| a.date <=> b.date}

    @players = @team.team_players.where(championship_id: @championship.id).includes(:player).to_a
    player_games = PlayerGame.where(player_id: @players.map{|p|p.player_id}, team_id: @team.id, game_id: @played_games)
    player_goals = Goal.where(player_id: @players.map{|p|p.player_id}, game: @played_games, team_id: @team.id)
    @players.sort!{|a,b| a.player.name <=> b.player.name}.map! do |p|
      goals = player_goals.select{|player|player.player_id == p.player_id}
      games = player_games.select{|player|player.player_id == p.player_id}
      { :player => p.player,
        :team_player => p,
        :goals => goals.select{|g|g.own_goal == false}.size,
        :penalties => goals.select{|g|g.penalty == true}.size,
        :own_goals => goals.select{|g|g.own_goal == true}.size,
        :appearances => games.select{|pg| pg.off > 0}.size,
        :bench => games.select{|pg| pg.off == 0 and pg.on == 0}.size,
        :sub => games.select{|pg| pg.on > 0}.size,
        :reds => games.select{|pg| pg.red}.size,
        :yellows => games.select{|pg| pg.yellow}.size,
        :minutes => games.map{|pg| pg.off - pg.on}.sum,
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
    @games = @championship.games.reorder("attendance DESC").page(params[:page]).per_page(10)
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

  def spi_eval
    @championships = Championship.where(id: params["id"])
    start_date = Game.joins(phase: :championship).where(championships: { id: @championships }).order(:date).first.date
    end_date = Game.joins(phase: :championship).where(championships: { id: @championships }).order(:date).last.date
    all_games = Game.joins(phase: :championship).select(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field).where(championships: { category_id: 1 }, played: true).where("date > ?", start_date - 4.years).where("date <= ?", end_date).order(:date)
    json_map = { phases_to_eval: @championships.map{|c|c.phases}.flatten.map{|p|p.id},
            games: all_games.pluck(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field)
                    .map{|home_id, away_id, phase_id, home_score, home_aet, away_score, away_aet, date, home_field|
        { home_id: home_id,
          away_id: away_id,
          phase_id: phase_id,
          home_score: (home_score + home_aet.to_i).to_f / if home_aet.nil? then 1.0 else 4.0/3.0 end,
          away_score: (away_score + away_aet.to_i).to_f / if home_aet.nil? then 1.0 else 4.0/3.0 end,
          timestamp: date.to_i,
          length: if home_aet.nil? then 1.0 else 4.0/3.0 end,
          advantage: if home_field == Game.home_fields["left"] then Game::HOME_ADV elsif home_field == Game.home_fields["neutral"] then 0.0 else -Game::HOME_ADV end }
    }, ratings: Team.all.pluck(:id, :off_rating, :def_rating).map{|id, off_rating, def_rating| {id: id, offense: off_rating, defense: def_rating} }}
    req = Net::HTTP::Post.new("/eval", {'Content-Type' =>'application/json'})
    req.body = Oj.dump(json_map, mode: :compat)
    response = Net::HTTP.new("localhost", 6577).start {|http| http.read_timeout = 300; http.request(req) }
    render plain: ActiveSupport::JSON.decode(response.body)
  end

  private
  def championship_params
    params.require(:championship).permit(:name, :begin, :end, :point_win, :point_draw, :point_loss, :category_id, :show_country, :region, :region_name)
  end

  def phase_params
    params.require(:phase).permit(:name, :order_by)
  end
end
