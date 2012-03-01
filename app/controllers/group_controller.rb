class GroupController < ApplicationController
  N_("Group")

  authorize_resource

  def edit
    @group = Group.find(params["id"])
    @teams = Team.all(:order => :name)
  end

  def update
    @group = Group.find(params["id"])
    @group.attributes = params["group"]

    valid = @group.valid?

	@group.team_groups.clear

    new_team_groups = Array.new
    params["team_group"].each do |key, value|
      value["comment"] = nil if value["comment"].to_s.empty?
      new_team_groups.push TeamGroup.new(value.merge({:group_id => @group.id}))
      valid = new_team_groups.last.try(:valid?) && valid
    end unless params["team_group"].nil?

	@group.team_groups = new_team_groups

    if valid
      @group.save!
      redirect_to :controller => :championship, :action => :phases, :id => @group.phase.championship, :phase => @group.phase
    else
      @teams = Team.all(:order => :name)
      render :action => "edit"
    end
  end

  def destroy
    group = Group.find(params["id"])
    group.destroy
    redirect_to :controller => :phase, :action => :edit, :id => group.phase
  end

  def update_odds
    @group = Group.find(params["id"])
    if @group.odds_progress == nil
      @group.send_later :odds
      @group.odds_progress = 0
      @group.save!
    elsif @group.odds_progress == 100
      @group.odds_progress = nil
      @group.save!
    end
  end
end
