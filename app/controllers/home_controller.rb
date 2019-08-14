class HomeController < ApplicationController
  N_("Home")

  authorize_resource :class => false

  def index
    today = cookie_timezone.today
    @championships = Championship.where("begin <= ? AND end >= ?", today+30, today-30).includes(:teams).sort_by{|a| -a.avg_team_rating }
    @regions = Championship.regions.keys
    @comments = Comment.includes(:user).order("created_at DESC").limit(5)
    @games = Game.includes(:home, :away, :updater, {phase: :championship}).where("updater_id != 0").order(updated_at: :desc).limit(5)
    @top_games = Game.includes(:home, :away).select("games.*, 2*teams.rating*aways_games.rating/(teams.rating+aways_games.rating)*(1+(IFNULL(home_importance,0)+IFNULL(away_importance,0))/2) as quality").order("quality desc, date desc").where(played: false).where("date > ?", Time.now - 3.hours).where("date < ?", Time.now + 1.day - 3.hours).where(has_time: true).limit(20).references(:home, :away)
    top_quality_championship = {}
    @top_games.each do |g|
      top_quality_championship[g.phase.id] = g.game_quality if g.game_quality > top_quality_championship[g.phase.id].to_f
    end
    @top_games = @top_games.sort_by{|a|[a.date.in_time_zone(cookie_timezone).to_date, -top_quality_championship[a.phase.id], a.date, -a.game_quality]}
  end
end
