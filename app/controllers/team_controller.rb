class TeamController < ApplicationController
  scaffold :team

  def show
    @team = Team.find(@params["id"])
  end

  def find
    unless @params["id"].nil?
      teams = Team.find(:all, :conditions => [ "name like ?", @params["id"]],
                        :order => 'name')
      @team_pages = Paginator.new self, teams.size, 10, @params["page"]
      first = @team_pages.current.offset
      last = [first + 10, teams.size].min
      @teams = teams[first...last]
      render :action => "list"
    end
  end
end
