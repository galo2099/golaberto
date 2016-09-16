class HomeController < ApplicationController
  N_("Home")

  authorize_resource :class => false

  def index
    @today = cookie_timezone.today
    @championships = Championship.where("begin <= ? AND end >= ?", @today+30, @today-30).
        order("category_id, name, begin").includes(phases: :teams)
    @championships = @championships.all.sort{|b,a| a.avg_team_rating <=> b.avg_team_rating}
    @comments = Comment.order("created_at DESC").limit(5)
    @games = Game.where("updater_id != 0").order("updated_at DESC").limit(5)
  end
end
