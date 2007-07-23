class TeamGroupController < ApplicationController
  scaffold :team_group
  before_filter :login_required, :except => [ :index, :list, :show ]

  def index
    list
    render :action => 'list'
  end

  def list
    @team_group_pages, @team_groups = paginate :team_groups, :per_page => 10
  end
end
