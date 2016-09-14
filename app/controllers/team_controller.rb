class TeamController < ApplicationController
  include ApplicationHelper

  N_("Team")

  authorize_resource

  def index
    redirect_to :action => :list
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

    @teams = Team.order(:name).where(conditions).page(params[:page])
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
    team = Team.find(params[:id]).destroy
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
