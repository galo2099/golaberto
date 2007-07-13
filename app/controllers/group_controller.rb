class GroupController < ApplicationController
  scaffold :group

  def edit
    @group = Group.find(@params["id"])
    @team_number = @params["team_number"]
    @team_number = @group.teams.size if @team_number.nil?
    @team_number = @team_number.to_i
    if request.xhr?
      render :partial => "team_list"
    end
  end

  def update
    @group = Group.find(@params["id"])
    @group.attributes = @params["group"]

    saved = @group.save

    @group.team_groups.clear
    @teams = Array.new
    @params["team_group"].each do |key, value|
      value["comment"] = nil if value["comment"].to_s.empty?
      @teams.push @group.team_groups.build(value)
      saved = @teams.last.save && saved
    end unless @params["team_group"].nil?

    @team_number = @group.team_groups

    if saved
      redirect_to :controller => :championship, :action => :phases, :id => @group.phase.championship, :phase => @group.phase
    else
      render :action => "edit" 
    end
  end

  def destroy
    Group.find(@params["id"]).destroy
    redirect_to :back
  end
end
