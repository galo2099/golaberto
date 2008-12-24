class RefereeController < ApplicationController
  N_("Referee")

  require_role "editor", :except => [ :index, :list, :show ]

  def index
    redirect_to :action => :list
  end

  def list
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @referees = Referee.paginate :order => "name",
                                 :conditions => conditions,
                                 :page => params[:page]
  end

  def show
    store_location
    @referee = Referee.find(params[:id])
  end

  def edit
    @referee = Referee.find(params["id"])
  end

  def update
    @referee = Referee.find(params[:id])
    @referee.attributes = params[:referee]

    if @referee.save
      flash[:notice] = _("Referee was successfully updated")
      redirect_to :action => :show, :id => @referee
    else
      render :action => :edit
    end
  end

  def new
    @referee = Referee.new
  end

  def create
    @referee = Referee.new(params[:referee])
    if @referee.save
      flash[:notice] = _("Referee was successfully created")
      redirect_to :action => :show, :id => @referee
    else
      render :action => :new
    end
  end

  def destroy
    Referee.find(params[:id]).destroy
    redirect_to :action => :list
  end
end
