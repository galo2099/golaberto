class TeamController < ApplicationController
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def show
    store_location
    @team = Team.find(params["id"])
    @championships = @team.team_groups.map{|t| t.group.phase.championship}.uniq.sort{|a,b| b.begin <=> a.begin}
  end

  def edit
    @team = Team.find(params["id"])
    @stadiums = Stadium.find(:all, :order => :name)
  end

  def update
    @team = Team.find(params["id"])
    @team.attributes = params["team"]

    begin
      @team.save!
      @team.uploaded_logo(params[:logo], params[:filter]) unless params[:logo].to_s.empty?
      redirect_to :action => :show, :id => @team
    rescue
      @stadiums = Stadium.find(:all, :order => :name)
      render :action => :edit
    end
  end

  def list
    items_per_page = 10
    conditions = ["name LIKE ?", "%#{params[:id]}%"] unless params[:id].nil?

    @total = Team.count :conditions => conditions
    @team_pages, @teams = paginate :teams, :order => "name",
                                   :conditions => conditions,
                                   :per_page => items_per_page

    if request.xhr?
      render :partial => "team_list", :layout => false
    end
  end

  def new
    @team = Team.new
    @stadiums = Stadium.find(:all, :order => :name)
  end

  def create
    @team = Team.new(params[:team])
    begin
      @team.save!
      @team.uploaded_logo(params[:logo], params[:filter]) unless params[:logo].to_s.empty?
      redirect_to :action => :show, :id => @team
    rescue
      @stadiums = Stadium.find(:all, :order => :name)
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
end
