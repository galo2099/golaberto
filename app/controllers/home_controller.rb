class HomeController < ApplicationController
  N_("Home")

  authorize_resource :class => false

  def index
    now = DateTime.now - 3.hours
    today = cookie_timezone.today
    @championships = Championship.where("begin <= ? AND end >= ?", today+30, today-30).includes(:teams).sort_by{|a| -a.avg_team_rating }
    @regions = Championship.regions.keys
    @comments = Comment.includes(:user).order("created_at DESC").limit(5)
    @games = Game.includes(:home, :away, :updater, {phase: :championship}).where("updater_id != 0").order(updated_at: :desc).limit(5)
    @games_to_highlight = []
    @top_games, to_highlight = games_by_quality(Game.includes(:home, :away, {phase: :championship}).select("games.*, 2*teams.rating*aways_games.rating/(teams.rating+aways_games.rating)*(1+(IFNULL(home_importance,0)+IFNULL(away_importance,0))/2) as quality").order("quality desc, date desc").where(played: false).where("date > ?", now).where("date < ?", now + 7.days).where(has_time: true).references(:home, :away), now)
    @games_to_highlight.push(*to_highlight)
    @top_played_games, to_highlight = played_games_by_quality(Game.includes(:home, :away, {phase: :championship}).select("games.*, 2*teams.rating*aways_games.rating/(teams.rating+aways_games.rating)*(1+(IFNULL(home_importance,0)+IFNULL(away_importance,0))/2) as quality").order("quality desc, date desc").where(played: true).where("date > ?", now - 7.days).where("date < ?", now + 7.days).where(has_time: true).references(:home, :away), now)
    @games_to_highlight.push(*to_highlight)
  end

  private

  def played_games_by_quality(games, now)
    games_by_quality_impl(games, now, true)
  end

  def games_by_quality(games, now)
    games_by_quality_impl(games, now, false)
  end

  def games_by_quality_impl(games, now, played)
    top_quality_championship = {}
    games_to_highlight = {}
    game_quality = {}
    games.each do |g|
      game_quality[g.id] = g.game_quality / 2.0**(((g.date - now)**2)**0.5/24/60/60)
    end
    games.each do |g|
      if game_quality[g.id] >= top_quality_championship[[g.phase.id, g.date.in_time_zone(cookie_timezone).to_date]].to_f
        top_quality_championship[[g.phase_id, g.date.in_time_zone(cookie_timezone).to_date]] = game_quality[g.id] 
        games_to_highlight[[g.phase_id, g.date.in_time_zone(cookie_timezone).to_date]] = g.id
      end
    end
    games = games.sort_by{|a|[-game_quality[a.id]]}[0,20]
    if played
      games = games.sort_by{|a|[-a.date.in_time_zone(cookie_timezone).to_date.to_time.to_i, -top_quality_championship[[a.phase_id, a.date.in_time_zone(cookie_timezone).to_date]], -a.date.to_i, -game_quality[a.id]]}[0,20]
    else
      games = games.sort_by{|a|[a.date.in_time_zone(cookie_timezone).to_date, -top_quality_championship[[a.phase_id, a.date.in_time_zone(cookie_timezone).to_date]], a.date, -game_quality[a.id]]}[0,20]
    end
    return games, games_to_highlight.values
  end
end
