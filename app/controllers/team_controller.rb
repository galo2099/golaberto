class TeamController < ApplicationController
  N_("Team")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def show
    store_location
    @team = Team.find(params["id"])
    @championships = @team.team_groups.map{|t| t.group.phase.championship}.uniq.sort{|a,b| b.begin <=> a.begin}
    @next_games = @team.next_n_games(5, { :date => Date.today })
    @last_games = @team.last_n_games(5, { :date => Date.today })
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
    rescue
      @stadiums = Stadium.order(:name)
      render :action => :edit
    end
  end

  def list
    @id = params[:id]
    conditions = ["name LIKE ?", "%#{@id}%"] unless @id.nil?

    @teams = Team.order(:name).where(conditions).page(params[:page])
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
