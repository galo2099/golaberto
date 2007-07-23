class PhaseController < ApplicationController
  before_filter :login_required

  def new
    @phase = Phase.new
  end

  def create
    @phase = Phase.new(@params["phase"])

    if @phase.save
      redirect_to :action => :show, :id => @phase
    else
      render :action => :new
    end
  end

  def edit
    @phase = Phase.find(@params["id"])
  end

  def update
    @phase = Phase.find(@params["id"])
    @phase.attributes = @params["phase"]

    saved = @phase.save
    new_empty = false

    @group = @phase.groups.build(@params["group"])
    new_empty = @group.name.empty?

    saved = @group.save unless new_empty

    if saved and new_empty
      redirect_to :controller => :championship, :action => :phases, :id => @phase.championship, :phase => @phase
    else
      render :action => "edit" 
    end
  end

  def destroy
    Phase.find(@params["id"]).destroy
    redirect_to :back
  end
end
