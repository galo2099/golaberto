class GroupController < ApplicationController
  N_("Group")

  skip_before_filter :verify_authenticity_token, :only => [:team_list]
  authorize_resource

  def edit
    @group = Group.find(params["id"])
  end

  def team_list
    @teams = Team.order(:name)
    respond_to do |format|
      format.js
    end
  end

  def update
    @group = Group.find(params["id"])
    @group.update_attributes(group_params)

	  @group.team_groups.clear

    params["team_group"].each do |key, value|
      value = value.permit(:team_id, :add_sub, :bias, :comment)
      value["comment"] = nil if value["comment"].to_s.empty?
      @group.team_groups << TeamGroup.new(value.merge({:group_id => @group.id}))
    end unless params["team_group"].nil?

    begin
      @group.save!
      redirect_to :controller => :championship, :action => :phases, :id => @group.phase.championship, :phase => @group.phase
    rescue
      @teams = Team.order(:name)
      render :action => "edit"
    end
  end

  def destroy
    group = Group.find(params["id"])
    group.destroy
    redirect_to :controller => :phase, :action => :edit, :id => group.phase
  end

  def odds_progress
    @group = Group.find(params["id"])
    if @group.odds_progress == 100 or @group.odds_progress.nil?
      @group.odds_progress = nil
      @group.save!
    end
  end

  def update_odds
    @group = Group.find(params["id"])
    if @group.odds_progress == nil
      @group.odds_progress = 0
      @group.save!
      Thread.new do
        ActiveRecord::Base.connection_pool.with_connection do
          @group.odds
        end
      end
    end
    render :action => :odds_progress
  end

  private
  def group_params
    params.require(:group).permit(:name, :promoted, :relegated)
  end
end
