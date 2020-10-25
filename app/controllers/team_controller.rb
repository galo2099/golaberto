class TeamController < ApplicationController
  include ApplicationHelper

  N_("Team")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def historic_ratings
    all_games = Game.joins(phase: :championship).select(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field).where(championships: { category_id: 1 }, played: true).where("date >= ?", DateTime.now - 5.years).where("date <= ?", DateTime.now).reorder(:date)
    json_map = { games: all_games.pluck(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field)
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
    req = Net::HTTP::Post.new("/historic_ratings", {'Content-Type' =>'application/json'})
    req.body = Oj.dump(json_map, mode: :compat)
    response = Net::HTTP.new("localhost", 6577).start {|http| http.read_timeout = 300; http.request(req) }
    ratings = ActiveSupport::JSON.decode(response.body)
    sql = "INSERT INTO historical_ratings (team_id,rating,measure_date) VALUES ";
    dates = ratings["dates"]
    ratings["ratings"].each do |k,v|
      v.each_with_index do |rating, i|
        sql << "(#{k}, #{rating}, '#{ratings["dates"][i].to_date.strftime(Date::DATE_FORMATS[:db])}'),"
      end
    end
    sql.chop!
    sql << "ON DUPLICATE KEY UPDATE rating=VALUES(rating);"
    ActiveRecord::Base.connection.execute(sql)
    redirect_to :back
  end

  def update_rating
    all_games = Game.joins(phase: :championship).select(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field).where(championships: { category_id: 1 }, played: true).where("date >= ?", DateTime.now - 4.years).where("date <= ?", DateTime.now).reorder(:date)
    json_map = { games: all_games.pluck(:home_id, :away_id, :phase_id, :home_score, :home_aet, :away_score, :away_aet, :date, :home_field)
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
    req = Net::HTTP::Post.new("/spi", {'Content-Type' =>'application/json'})
    req.body = Oj.dump(json_map, mode: :compat)
    response = Net::HTTP.new("localhost", 6577).start {|http| http.read_timeout = 300; http.request(req) }
    sql = "INSERT INTO teams (id,off_rating,def_rating,rating,created_at,updated_at) VALUES ";
    sql2 = "INSERT INTO historical_ratings (team_id,rating,measure_date) VALUES ";
    now = Time.zone.now.to_s.chop.chop.chop.chop
    Oj.load(response.body, bigdecimal_load: :float).each do |k,v|
      sql << "(#{k}, #{v ? v["Offense"] : "NULL"}, #{v ? v["Defense"] : "NULL"}, #{v ? v["Team"] : "NULL"}, '#{now}', '#{now}'),"
      if v != nil then
         sql2 << "(#{k}, #{v["Team"]}, '#{Date.today.strftime(Date::DATE_FORMATS[:db])}'),"
      end
    end
    sql.chop!
    sql2.chop!
    sql << "ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at);"
    sql2 << "ON DUPLICATE KEY UPDATE rating=VALUES(rating);"
    ActiveRecord::Base.connection.execute(sql)
    ActiveRecord::Base.connection.execute(sql2)
    redirect_to :back
  end

  def show
    store_location
    @team = Team.find(params["id"])
    @championships = Championship.joins(phases: {groups: :team_groups}).where(team_groups: {team_id: @team}).order(begin: :desc).uniq
    @next_games = @team.next_n_games(5, cookie_timezone.now).includes({phase: :championship}, :home, :away)
    @last_games = @team.last_n_games(5, cookie_timezone.now).includes({phase: :championship}, :home, :away).reverse
  end

  def edit
    @team = Team.find(params["id"])
    @stadiums = Stadium.order(:name)
  end

  def games
    store_location
    @categories = Category.all
    @team = Team.find(params["id"])
    @category = Category.find(params[:category])
    @type = params[:type].to_s
    @played = @type == "played"
    order = !!@played ? "date DESC" : "date ASC"
    @games = Game.team_games(@team).includes(:phase => :championship).where("played = ? and championships.category_id = ?", @played, @category).order(order).page(params[:page]).references(:championship)
  end

  def update
    @team = Team.find(params[:id])
    begin
      @team.attributes = team_params
      @team.save!
      redirect_to :action => :show, :id => @team
    rescue => e
      flash[:notice] = e.message
      @stadiums = Stadium.order(:name)
      render :action => :edit
    end
  end

  def list
    @id = params[:id]
    @country = params[:country]
    @team_type = params[:team_type] || "club"
    @continent = params[:continent] || ""
    teams = Team.where(team_type: Team.team_types[@team_type])
    if @id
      teams = teams.where(["name LIKE ?", "%#{@id}%"])
    end

    # Hash from country to number of teams
    countries_with_teams = { @country => 0 }
    countries_with_teams.merge!(teams.group(:country).size)

    countries_found = []
    countries_not_found = []
    golaberto_options_for_country_select.each do |translated_country, original_country|
      count = countries_with_teams[original_country]
      if count.nil? then
        countries_not_found << [translated_country, original_country]
      else
        countries_found << [translated_country + " (#{count})", original_country]
      end
    end

    @country_list = [[s_("Country|All") + " (#{teams.size})", ""]] + countries_found + countries_not_found
    @countries = {}
    @countries[""] = @country_list
    ApplicationHelper::Continent::ALL.each do |name, c|
      @countries[name] = [[s_("Country|All") + " (#{teams.where(country: c.countries.map{|c|c.name}).size})", ""]] + @country_list.select{|_, n| ApplicationHelper::Continent.country_to_continent[n] == c}
    end

    unless @continent.blank?
      teams = teams.where(country: Continent::ALL[@continent].countries.map{|c|c.name})
      unless Continent::ALL[@continent].countries.map{|c|c.name}.include? @country
        @country = nil
      end
    end

    @teams = teams.order(rating: :desc, name: :asc).page(params[:page])
    @teams = @teams.where(country: @country) unless @country.blank?
    if @teams.size == 1
      redirect_to :action => :show, :id => @teams.first
    end
    @teams = @teams.includes(:historical_ratings)
  end

  def new
    @team = Team.new
    @stadiums = Stadium.order(:name)
  end

  def create
    @team = Team.new
    begin
      @team.attributes = team_params
      @team.save!
      redirect_to :action => :show, :id => @team
    rescue
      @stadiums = Stadium.order(:name)
      render :action => :new
    end
  end

  def destroy
    Team.find(params[:id]).destroy
    redirect_to :action => :list
  end

  def auto_complete_for_team_name
    search = params[:team][:name]
    @teams = Team.find(
        :all,
        :conditions => "name like '#{search}%'",
        :order => "name",
        :limit => 5) unless search.blank?
    render :partial => "search"
  end

  private
  def team_params
    params.require(:team).permit(:name, :full_name, :city, :foundation, :country, :stadium_id, :filter_image_background, :logo, :team_type)
  end
end
