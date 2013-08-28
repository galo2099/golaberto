class PhaseController < ApplicationController
  N_("Phase")

  authorize_resource

  def new
    @phase = Phase.new
  end

  def create
    @phase = Phase.new(phase_params)

    if @phase.save
      redirect_to :action => :show, :id => @phase
    else
      render :action => :new
    end
  end

  def edit
    @phase = Phase.find(params["id"])
  end

  def add_groups
    phase = Phase.find(params["id"])
    last_group = phase.groups[-1]
    4.times do
      tokens = last_group.name.split(" ")
      tokens[-1].succ!
      new_name = tokens.join " "
      last_group = phase.groups.build
      last_group.name = new_name
      last_group.save!
    end
    render :partial => "group_list", :object => phase
  end

  def update
    @phase = Phase.find(params["id"])
    @phase.attributes = phase_params

    saved = @phase.save
    new_empty = false

    @group = @phase.groups.build(group_params)
    new_empty = @group.name.empty?

    saved = @group.save unless new_empty

    if saved and new_empty
      redirect_to :controller => :championship, :action => :phases, :id => @phase.championship, :phase => @phase
    else
      render :action => "edit"
    end
  end

  def destroy
    phase = Phase.find(params["id"])
    phase.destroy
    redirect_to :controller => :championship, :action => :edit, :id => phase.championship
  end

  private
  def phase_params
    params.require(:phase).permit(:name, :order_by, :sort, :bonus_points, :bonus_points_threshold)
  end

  def group_params
    params.require(:group).permit(:name)
  end
end
