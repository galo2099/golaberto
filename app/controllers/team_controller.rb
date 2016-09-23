class TeamController < ApplicationController
  include ApplicationHelper

  N_("Team")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def update_rating
    all_games = Game.joins(phase: :championship).where(championships: { category_id: 1 }, played: true).where("date > ?", Date.today - 4.years).all
    req = Net::HTTP::Post.new("/spi", {'Content-Type' =>'application/json'})
    teams = Team.where.not(off_rating: nil)
    req.body = { games: all_games.map{|g| [ g.home_id, g.away_id, g.home_score, g.away_score, g.date.to_i ]},
                 ratings: Hash[teams.map{|t| [t.id, [t.off_rating, t.def_rating]]}]}.to_json
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
    @championships = @team.team_groups.map{|t| t.group.phase.championship}.uniq.sort{|a,b| b.begin <=> a.begin}
    @next_games = @team.next_n_games(5, { :date => cookie_timezone.today })
    @last_games = @team.last_n_games(5, { :date => cookie_timezone.today })
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
    conditions = ["name LIKE ?", "%#{@id}%"] unless @id.nil?

    # Hash from country to number of teams
    countries_with_teams = { @country => 0 }
    countries_with_teams.merge!(Team.where(conditions).group(:country).size)

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

    @country_list = [[_("All") + " (#{Team.where(conditions).size})", ""]] + countries_found + countries_not_found

    @teams = Team.order(rating: :desc, name: :asc).where(conditions).page(params[:page])
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
    params.require(:team).permit(:name, :full_name, :city, :foundation, :country, :stadium_id, :filter_image_background, :logo)
  end
end
