class StadiumController < ApplicationController
  scaffold :stadium
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def list
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @stadia = Stadium.paginate :order => "name",
                               :conditions => conditions,
                               :page => params[:page]
  end

  def show
    store_location
    @stadium = Stadium.find(params[:id])
  end

  def edit
    @stadium = Stadium.find(params[:id])
  end
end
