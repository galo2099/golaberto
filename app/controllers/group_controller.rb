class GroupController < ApplicationController
  N_("Group")

  require_role "editor"

  def edit
    @group = Group.find(params["id"])
    @team_number = params["team_number"]
    @team_number = @group.teams.size if @team_number.nil?
    @team_number = @team_number.to_i
    @teams = Team.all(:order => :name)
    if request.xhr?
      render :partial => "team_list"
    end
  end

  def update
    @group = Group.find(params["id"])
    @group.attributes = params["group"]

    saved = @group.save

    @group.team_groups.clear
    @team_groups = Array.new
    params["team_group"].each do |key, value|
      value["comment"] = nil if value["comment"].to_s.empty?
      @team_groups.push @group.team_groups.build(value)
      saved = @team_groups.last.save && saved
    end unless params["team_group"].nil?

    @team_number = @group.team_groups.size

    if saved
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
