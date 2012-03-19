require 'poisson'
class Group < ActiveRecord::Base
  belongs_to :phase, :touch => true
  has_many :games, :through => :phase, :conditions => Proc.new {[ "home_id IN (?) OR away_id IN (?)", teams, teams ]}
  has_many :team_groups, :dependent => :delete_all, :include => :team
  has_many :teams, :through => :team_groups
  validates_length_of :name, :within => 1..40
  validates_uniqueness_of :name, :scope => :phase_id
  validates_numericality_of :promoted, :only_integer => true
  validates_numericality_of :relegated, :only_integer => true

  # Fields information, just FYI.
  #
  # Field: id , SQL Definition:bigint(20)
  # Field: phase_id , SQL Definition:bigint(20)
  # Field: name , SQL Definition:varchar(255)
  # Field: promoted , SQL Definition:tinyint(2)
  # Field: relegated , SQL Definition:tinyint(2)

  NUM_ITER = 10000

  def is_promoted?(pos)
    return pos <= self.promoted
  end

  def odds
    games_to_play = games.where(:played => false)
    games_played = games.where(:played => true)
    phase.sort.sub!("name", "rand")
    poisson_hash = Hash.new{|h,k| h[k] = Poisson.new(k)}
    odds = games_to_play.map do |g|
      [ g.home_id,
        g.away_id,
        poisson_hash[Poisson.find_mean_from(g.home_for.zip(g.away_against).map{|a,b| a+b})],
        poisson_hash[Poisson.find_mean_from(g.home_against.zip(g.away_for).map{|a,b| a+b})] ]
    end
    results = Hash.new{|h,k| h[k] = Hash.new{|hh,kk| hh[kk] = 0}}
    fixed_stats = Hash.new
    team_groups.each do |team_group|
      fixed_stats[team_group.team_id] =
          ChampionshipHelper::TeamCampaign.new(team_group)
    end
    games_played.each do |g|
      fixed_stats[g.home_id].add_game g if fixed_stats[g.home_id]
      fixed_stats[g.away_id].add_game g if fixed_stats[g.away_id]
    end
    NUM_ITER.times do |count|
      if count % (NUM_ITER / 100) == 0
        self.odds_progress = count / (NUM_ITER / 100)
        self.save!
      end
      stats = Hash.new
      fixed_stats.each do |k,v|
        stats[k] = v.clone
      end
      odds.each do |o|
        home_score = o[2].rand
        away_score = o[3].rand
        stats[o[0]].add_game_score_only o[0], o[1], home_score, away_score if stats[o[0]]
        stats[o[1]].add_game_score_only o[0], o[1], home_score, away_score if stats[o[1]]
      end
      sort_teams(stats).each_with_index do |v,i|
        if i == 0 then
          results[:champ][v[0].team_id] += 1
        end
        if i < promoted then
          results[:prom][v[0].team_id] += 1
        end
        if i >= stats.size - relegated then
          results[:rele][v[0].team_id] += 1
        end
      end
    end

    team_groups.each do |t|
      t.first_odds = results[:champ][t.team_id].to_f * 100 / NUM_ITER
      t.promoted_odds = results[:prom][t.team_id].to_f * 100 / NUM_ITER
      t.relegated_odds = results[:rele][t.team_id].to_f * 100 / NUM_ITER
      t.save!
    end
    self.odds_progress = 100
    self.save!
  end

  def is_relegated?(pos)
    return pos > teams.size - self.relegated
  end

  def create_home_and_away_games
    teams.each do |home|
      (teams - [home]).each do |away|
        phase.games.build(:home => home, :away => away, :played => "0", :date => Date.today).save!
      end
    end
  end

  def team_table
    @team_table ||= begin
      played_games = games.where("played = ?", true).
          includes(:phase, [:phase => :championship ]).
          order(:date)
      stats = Hash.new
      team_groups.each do |team_group|
        stats[team_group.team.id] =
            ChampionshipHelper::TeamCampaign.new(team_group)
      end
      last_round = nil
      last_date = nil
      last_games = Array.new
      played_games.each do |g|
        if (last_round and last_round != g.round) or
           (!last_round and last_date and last_date != g.date)
          yield sort_teams(stats), last_games if block_given?
          last_games = Array.new
        end
        last_date = g.date
        last_round = g.round
        last_games << g
        stats.each_value do |s|
          s.add_game g
        end
      end
      ret = sort_teams(stats)
      yield ret, last_games if block_given?
      ret
    end
  end

  def get_game(team_class, a, b)
    game = team_class[a].home_games[b]
    if game
      [ a, b, game[0], game[1] ]
    end
  end

  def compare_head_to_head(team_class, a, b)
    tied_teams = team_class.select { |k,v| v.points == team_class[a].points }.map{|k,v| k}
    if (tied_teams.size == team_class.size) then
      return 0
    end
    games = Array.new
    games << get_game(team_class, a, b)
    games << get_game(team_class, b, a)
    (tied_teams - [a,b]).each do |t|
      games << get_game(team_class, a, t)
      games << get_game(team_class, t, a)
      games << get_game(team_class, b, t)
      games << get_game(team_class, t, b)
    end
    games = games.select{|g|!g.nil?}
    sub_team_class = Hash.new
    tied_teams.each do |t|
      sub_team_class[t] = ChampionshipHelper::TeamCampaign.new(team_class[t].team_group)
    end
    games.each do |home_id, away_id, home_score, away_score|
      sub_team_class.values.each do |stat|
        stat.add_game_score_only home_id, away_id, home_score, away_score
      end
    end
    compare_teams(sub_team_class, @columns - ['name'], a, b)
  end

  def compare_teams(team_class, columns, a, b)
    ret = 0
    columns.detect do |column|
      case column
      when "pt"
        ret = team_class[b].points <=> team_class[a].points
      when "w"
        ret = team_class[b].wins <=> team_class[a].wins
      when  "gd"
        ret = team_class[b].goals_diff <=> team_class[a].goals_diff
      when "gf"
        ret = team_class[b].goals_for <=> team_class[a].goals_for
      when "name"
        ret = team_class[a].name <=> team_class[b].name
      when "g_average"
        ret = team_class[b].goals_avg <=> team_class[a].goals_avg
      when "gp"
        ret = team_class[b].goals_pen <=> team_class[a].goals_pen
      when "g_aet"
        ret = team_class[b].goals_aet <=> team_class[a].goals_aet
      when "head"
        ret = compare_head_to_head(team_class, a, b) if team_class.size > 2
      when "g_away"
        ret = team_class[b].goals_away <=> team_class[a].goals_away
      when "bias"
        ret = team_class[b].bias <=> team_class[a].bias
      when "rand"
        ret = 2*rand(2)-1
      end
      ret != 0
    end
    ret
  end

  def sort_teams(team_class)
    @columns ||= phase.sort.split(/,\s*/)
    team_class.keys.sort do |a,b|
      compare_teams team_class, @columns, a, b
    end.map do |t|
      [ team_class[t].team_group, team_class[t] ]
    end
  end
end
