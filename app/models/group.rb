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

  def is_promoted?(pos)
    return pos <= self.promoted
  end

  def is_relegated?(pos)
    return pos > teams.size - self.relegated
  end

  def team_table
    games = phase.games.find(:all,
        :conditions => [ "home_id in (?) OR away_id in (?)",
                         team_groups.map{|t|t.team_id},
                         team_groups.map{|t|t.team_id} ],
        :order => :date)
    stats = Array.new
    team_groups.each do |team_group|
      stats[team_group.team.id] =
          ChampionshipHelper::TeamCampaign.new(team_group, games)
    end
    sort_teams(stats)
  end

  private

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
          ret = team_groups.find(
              :first,
              :conditions => [ "team_id = ?", a.id ]).bias <=>
                team_groups.find(:first,
              :conditions => [ "team_id = ?", b.id ]).bias
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
