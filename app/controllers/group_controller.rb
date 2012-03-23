class GroupController < ApplicationController
  N_("Group")

  authorize_resource

  def edit
    @group = Group.find(params["id"])
    @teams = Team.order(:name)
  end

  def update
    @group = Group.find(params["id"])
    @group.update_attributes(params["group"])

	@group.team_groups.clear

    params["team_group"].each do |key, value|
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
    if @group.odds_progress == 100
      @group.odds_progress = nil
      @group.save!
    end
  end

  def update_odds
    @group = Group.find(params["id"])
    if @group.odds_progress == nil
      @group.odds_progress = 0
      @group.delay.odds
      @group.save!
    end
    render :action => :odds_progress
  end
end
