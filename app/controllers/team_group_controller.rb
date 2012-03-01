class TeamGroupController < ApplicationController
  N_("TeamGroup")

  scaffold :team_group
  authorize_resource

  def index
    list
    render :action => 'list'
  end

  def list
    @team_group_pages, @team_groups = paginate :team_groups, :per_page => 10
  end
end
