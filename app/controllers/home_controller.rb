class HomeController < ApplicationController
  N_("Home")

  authorize_resource :class => false

  def index
    today = cookie_timezone.today
    @championships = Championship.where("begin <= ? AND end >= ?", today+30, today-30).
        order("category_id, name, begin").includes(phases: :teams)
    @championships = @championships.all.sort_by{|a| -a.avg_team_rating }
    @continents = []
    @championships.each do |c|
      continent = ApplicationHelper::Continent.country_to_continent[c.teams[0].country]
      if not @continents.include? continent
        @continents << continent
      end
    end
    @comments = Comment.order("created_at DESC").limit(5)
    @games = Game.where("updater_id != 0").order("updated_at DESC").limit(5)
    @top_games = Game.joins(:home, :away).select("games.*, 2*teams.rating*aways_games.rating/(teams.rating+aways_games.rating) as quality").order("quality desc, date desc").where(played: false).where("date > ?", Time.now - 3.hours).where("date < ?", Time.now + 1.day - 3.hours).where(has_time: true).limit(20)
    @top_games = @top_games.to_a.sort_by{|a|[a.date.in_time_zone(cookie_timezone).to_date, -a.phase.avg_team_rating, a.date]}
  end
end
