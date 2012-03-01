class StadiumController < ApplicationController
  N_("Stadium")

  authorize_resource

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

  def update
    @stadium = Stadium.find(params[:id])
    @stadium.attributes = params[:stadium]

    if @stadium.save
      flash[:notice] = _("Stadium was successfully updated")
      redirect_to :action => :show, :id => @stadium
    else
      render :action => :edit
    end
  end

  def new
    @stadium = Stadium.new
  end

  def create
    @stadium = Stadium.new(params[:stadium])
    if @stadium.save
      flash[:notice] = _("Stadium was successfully created")
      redirect_to :action => :show, :id => @stadium
    else
      render :action => :new
    end
  end

  def destroy
    Stadium.find(params[:id]).destroy
    redirect_to :action => :list
  end
end
