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

    @pagy, @championships = pagy(@championships, items: 30)
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
    @championship = Championship.includes(phases: { teams: :team_geocode }).find(params[:id])
    # Use an empty phase instead of nil if none is passed.
    @current_phase = Phase.new
    @current_phase = @championship.phases.includes(teams: :team_geocode).find(params[:phase]) if params[:phase]
    if @current_phase
      @hide_odds = @current_phase.games.find_by_played(false) == nil
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
                colors: [ "#0000ff", "#696969", "#ff0000", "#000000", "#0000ff", "#696969", "#ff0000", "#b00baa" ],
                grid: {
                  backgroundColor: "#FFFFFF",
                  hoverable: true,
                  clickable: true,
                  markings: group.zones.reverse.select{|z| not z["position"].nil? }.map{|z| z["position"].map{|p| { yaxis: { from: p-0.5, to: p+0.5 }, color: z["color"] } } }.flatten,
                },
                xaxes: [
                  {
                    ticks: (1..data.size).to_a.map{|x|[x, ""]},
                    min: 0.5,
                    max: data.size + 0.5,
                    autoScale: false,
                  },
                  {
                    autoScale: false,
                    min: 0.5,
                    max: data.size + 0.5,
                  },
                ],
                yaxes: [
                  {
                    ticks: (1..group.team_groups.size).to_a.map{|x|[x, x.to_s]},
                    showTicks: false,
                    showTickLabels: "major",
                    autoScale: false,
                    min: 0.5,
                    max: group.team_groups.size + 0.5,
                  },
                  {
                    show: false,
                    showTicks: false,
                    min: 0,
                    max: points_for_1st_place + 1,
                    autoScale: false,
                  },
                ]
              },
              data: [],
              tooltips: [[], []],
              urls: [[], []],
            }

    bar = [
      { data: [], yaxis: 2, bars: { show: true, barWidth: [0.8, true], align: "center" }, label: nil },
      { data: [], yaxis: 2, bars: { show: true, barWidth: [0.8, true], align: "center" }, label: nil },
      { data: [], yaxis: 2, bars: { show: true, barWidth: [0.8, true], align: "center" }, label: nil },
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
             label: team.name,
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
    team = championship.teams.find(params["team"])
    phase = championship.phases.find(params["phase"])
    group = phase.groups.select{|g| g.teams.include? team}.first
    json, _ = generate_team_json(championship, phase, group, team)

    render json: json
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
    @team = @championship.teams.find(params["team"])

    # Find every group that this team belonged to
    @groups = @championship.phases.map{|p| p.groups}.flatten.select{|g| g.teams.include? @team}.reverse

    @group_json = []
    @odds_history_json = []
    @groups.each_with_index do |g, idx|
      json, _ = generate_team_json(@championship, g.phase, g, @team)
      @group_json << json
      @odds_history_json << generate_odds_history_json(g, @team)
    end

    @played_games = @team.home_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games += @team.away_games.where(phase_id: @championship.phase_ids, played: true).includes(:home, :away)
    @played_games.sort!{|a,b| a.date <=> b.date}

    @scheduled_games = @team.home_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games += @team.away_games.where(phase_id: @championship.phase_ids, played: false).includes(:home, :away)
    @scheduled_games.sort!{|a,b| a.date <=> b.date}

    @player_stats = TeamPlayer.stats(game: @played_games, team_id: @team.id).includes(:player)
  end

  def generate_odds_history_json(group, team)
    team_group = group.team_groups.find_by(team_id: team.id)
    return { series: [], has_data: false }.to_json if team_group.nil?

    zones = group.zones.is_a?(Array) ? group.zones.select { |z| z.is_a?(Hash) && z["position"].is_a?(Array) } : []
    snapshots = team_group.odds_histories.order(:recorded_on)

    history_by_day = snapshots.map do |snapshot|
      [snapshot.recorded_on, snapshot.captured_at || snapshot.recorded_on.end_of_day, snapshot.odds]
    end

    played_team_games = group.games
      .where(played: true)
      .where("home_id = :team_id OR away_id = :team_id", team_id: team.id)
      .includes(:home, :away)
      .order(:date, :id)
      .to_a

    latest_game_by_timestamp = {}
    game_idx = 0
    latest_game = nil
    history_by_day.each do |_, snapshot_time, _|
      while game_idx < played_team_games.size && played_team_games[game_idx].date && played_team_games[game_idx].date <= snapshot_time
        latest_game = played_team_games[game_idx]
        game_idx += 1
      end
      latest_game_by_timestamp[snapshot_time] = latest_game
    end

    zone_odds_by_snapshot = history_by_day.map do |_, _, odds|
      zones.map do |zone|
        positions = zone["position"].map(&:to_i).uniq
        value = positions.sum { |position| odds[position - 1].to_f }
        {
          name: zone["name"],
          color: zone["color"],
          value: [value, 100.0].min.round(2),
        }
      end
    end

    positions_count = group.team_groups.size
    color_by_position = {}
    zone_name_by_position = {}
    zone_names_by_position = {}
    (1..positions_count).each do |position|
      matching_zones = zones.select { |item| item["position"].map(&:to_i).include?(position) }
      zone = matching_zones.first
      color_by_position[position] = zone ? zone["color"] : "#ffffff"
      zone_name_by_position[position] = zone ? zone["name"] : nil
      zone_names_by_position[position] = matching_zones.map { |item| item["name"] }
    end

    series = positions_count.downto(1).map do |position|
      points = []
      points_meta = []

      history_by_day.each_with_index do |(recorded_on, snapshot_time, odds), history_index|
        next if odds.nil?

        value = odds[position - 1].to_f
        points << [recorded_on.to_time.to_i * 1000, [value, 100.0].min.round(4)]

        game = latest_game_by_timestamp[snapshot_time]
        points_meta << {
          recorded_on: recorded_on.to_s,
          game_date: game&.date&.to_s,
          game_label: if game
                        "#{game.home.name} #{game.home_score}-#{game.away_score} #{game.away.name}"
                      else
                        nil
                      end,
          zone_odds: zone_odds_by_snapshot[history_index],
        }
      end

      {
        label: position.ordinalize,
        zone_name: zone_name_by_position[position],
        zone_names: zone_names_by_position[position] || [],
        color: color_by_position[position],
        data: points,
        point_meta: points_meta,
      }
    end

    zone_legend = zones.map { |zone| { name: zone["name"], color: zone["color"] } }

    { series: series, zones: zone_legend, has_data: series.any? { |item| item[:data].any? } }.to_json
  end

  def player_list
    store_location
    @championship = Championship.includes(:phases => [ :teams, { :groups => :teams }]).find(params["id"])

    @player_stats = TeamPlayer.stats("games.phase_id": @championship.phases.pluck(:id)).includes(:player, :team, :game)
    @player_stats = @player_stats.to_a.sort{|a,b| b.goals <=> a.goals}
  end

  def player_show
    store_location
    @championship = Championship.find(params["id"])
    @team = @championship.teams.find(params[:team])
    @player = Player.find(params[:player])

    @player_stats = TeamPlayer.stats("games.phase_id": @championship.phases.pluck(:id), player: @player, team: @team)
    @player_stats = @player_stats.to_a.sort{|a,b| b.goals <=> a.goals}.first
    raise ActiveRecord::RecordNotFound if @player_stats.nil?
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

    @pagy, @games = pagy(games.order("date, round, teams.name").includes(:home, :away).references(:team), items: 30)
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
    @pagy, @games = pagy(@championship.games.reorder("attendance DESC"), items: 10)
  end

  def clone_phase
    @championship = Championship.find(params["id"])
    phase = @championship.phases.find(params["phase"])
    cloned_championship = @championship.clone_phase_to_new_championship!(phase)
    redirect_to :action => :phases, :id => cloned_championship, :phase => cloned_championship.phases.first
  end

  def destroy
    Championship.find(params["id"]).destroy
    redirect_to action: :list
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
