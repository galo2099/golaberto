class RefereeController < ApplicationController
  N_("Referee")

  authorize_resource

  def index
    redirect_to :action => :list
  end

  def list
    @name = params[:name]
    conditions = ["name LIKE ?", "%#{@name}%"] unless @name.nil?

    @referees = Referee.order(:name).where(conditions).page(params[:page])
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
    @referee.attributes = referee_params

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
    @referee = Referee.new(referee_params)
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

  private

  def referee_params
    params.require(:referee).permit(:name, :location)
  end
end
