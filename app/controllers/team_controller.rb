class TeamController < ApplicationController
  include ApplicationHelper

  N_("Team")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def update_rating
    all_games = Game.joins(phase: :championship).where(championships: { category_id: 1 }, played: true).where("date > ?", Date.today - 4.years).order(:date).all
    req = Net::HTTP::Post.new("/spi", {'Content-Type' =>'application/json'})
    teams = Team.where.not(off_rating: nil)
    req.body = { games: all_games.order(:date).map{|g|
        [ g.home_id,
          g.away_id,
          (g.home_score + g.home_aet.to_i).to_f / if g.home_aet.nil? then 1.0 else 4.0/3.0 end,
          (g.away_score + g.away_aet.to_i).to_f / if g.home_aet.nil? then 1.0 else 4.0/3.0 end,
          g.date.to_i,
          if g.home_aet.nil? then 1.0 else 4.0/3.0 end,
          g.left_advantage ]
    }, ratings: Hash[teams.map{|t| [t.id, [t.off_rating, t.def_rating]]}]}.to_json
    response = Net::HTTP.new("localhost", 6577).start {|http| http.read_timeout = 300; http.request(req) }
    sql = "INSERT INTO teams (id,off_rating,def_rating,rating,created_at,updated_at) VALUES ";
    now = Time.zone.now.to_s.chop.chop.chop.chop
    ActiveSupport::JSON.decode(response.body).each do |k,v|
      sql += "(#{k}, #{v["Offense"]}, #{v["Defense"]}, #{Team.calculate_rating2(v["Offense"], v["Defense"])}, '#{now}', '#{now}'),"
    end
    sql.chop!
    sql += "ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at);"
    ActiveRecord::Base.connection.execute(sql)
    redirect_to :back
  end

  def show
    store_location
    @team = Team.find(params["id"])
    @championships = Championship.joins(phases: {groups: :team_groups}).where(team_groups: {team_id: @team}).order(begin: :desc).uniq
    @next_games = @team.next_n_games(5, cookie_timezone.today).includes({phase: :championship}, :home, :away)
    @last_games = @team.last_n_games(5, cookie_timezone.today).includes({phase: :championship}, :home, :away).reverse
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
