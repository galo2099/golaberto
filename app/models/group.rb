class Group < ActiveRecord::Base
  belongs_to :phase
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
    games_to_play = phase.games.group_games(self).find(:all, :conditions => { :played => false })
    games_played = phase.games.group_games(self).find(:all, :conditions => { :played => true })
    phase.sort.sub!("name", "rand")
    poisson_hash = Hash.new{|h,k| h[k] = Poisson.new(k)}
    odds = games_to_play.map do |g|
      { :game => g,
        :home_id => g.home_id,
        :away_id => g.away_id,
        :home_power => poisson_hash[Poisson.find_mean_from(g.home_for.zip(g.away_against).map{|a,b| a+b})],
        :away_power => poisson_hash[Poisson.find_mean_from(g.home_against.zip(g.away_for).map{|a,b| a+b})] }
    end
    results = Hash.new{|h,k| h[k] = Hash.new{|hh,kk| hh[kk] = 0}}
    fixed_stats = Hash.new
    team_groups.each do |team_group|
      fixed_stats[team_group.team_id] =
          ChampionshipHelper::TeamCampaign.new(team_group)
    end
    games_played.each do |g|
      fixed_stats[g.home_id].add_game g
      fixed_stats[g.away_id].add_game g
    end
    NUM_ITER.times do |count|
      stats = Hash.new
      fixed_stats.each do |k,v|
        stats[k] = v.clone
      end
      odds.each do |o|
        o[:game].home_score = o[:home_power].rand
        o[:game].away_score = o[:away_power].rand
        stats[o[:home_id]].add_game o[:game]
        stats[o[:away_id]].add_game o[:game]
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
  end

  def is_relegated?(pos)
    return pos > teams.size - self.relegated
  end

  def team_table
    played_games = phase.games.group_games(self).find(:all,
        :conditions => [ "played = ?", true ],
        :include => [ :phase, [:phase => :championship ] ],
        :order => :date)
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
         (last_date and last_date != g.date)
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

  def sort_teams(team_class)
    columns = phase.sort.split(/,\s*/)
    sorter = lambda { |b,a|
      ret = 0
      columns.detect do |column|
        case column
        when "pt"
          ret = team_class[a.id].points <=> team_class[b.id].points
        when "w"
          ret = team_class[a.id].wins <=> team_class[b.id].wins
        when  "gd"
          ret = team_class[a.id].goals_diff <=> team_class[b.id].goals_diff
        when "gf"
          ret = team_class[a.id].goals_for <=> team_class[b.id].goals_for
        when "name"
          ret = a.name <=> b.name
        when "g_average"
          ret = team_class[a.id].goals_avg <=> team_class[b.id].goals_avg
        when "gp"
          ret = team_class[a.id].goals_pen <=> team_class[b.id].goals_pen
        when "g_away"
          ret = team_class[a.id].goals_away <=> team_class[b.id].goals_away
        when "bias"
          ret = team_class[a.id].bias <=> team_class[b.id].bias
        when "rand"
          ret = 2*rand(2)-1
        end
        ret != 0
      end
      ret
    }
    team_groups.sort do |a,b|
      sorter.call a.team, b.team
    end.map do |t|
      [ t, team_class[t.team.id] ]
    end
  end
end
