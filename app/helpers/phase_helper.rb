module PhaseHelper
  def sort_teams(phase, group, team_class)
    columns = phase.sort.split(/,\s*/)
    sorter = lambda { |b,a|
      ret = 0
      columns.each do |column|
        if column == "pt"
          ret = team_class[a.id].points <=> team_class[b.id].points
          break if ret != 0
        elsif column ==  "w"
          ret = team_class[a.id].wins <=> team_class[b.id].wins
          break if ret != 0
        elsif column ==  "gd"
          ret = team_class[a.id].goals_diff <=> team_class[b.id].goals_diff
          break if ret != 0
        elsif column ==  "gf"
          ret = team_class[a.id].goals_for <=> team_class[b.id].goals_for
          break if ret != 0
        elsif column ==  "name"
          ret = a.name <=> b.name
          break if ret != 0
        elsif column ==  "g_average"
          ret = team_class[a.id].goals_avg <=> team_class[b.id].goals_avg
          break if ret != 0
        elsif column ==  "gp"
          ret = team_class[a.id].goals_pen <=> team_class[b.id].goals_pen
          break if ret != 0
        elsif column ==  "g_away"
          ret = team_class[a.id].goals_away <=> team_class[b.id].goals_away
          break if ret != 0
        elsif column ==  "bias"
          ret = group.team_groups.find(
              :first,
              :conditions => [ "team_id = ?", a.id ]).bias <=>
                group.team_groups.find(:first,
              :conditions => [ "team_id = ?", b.id ]).bias
          break if ret != 0
        end
      end
      ret
    }
    group.team_groups.sort do |a,b|
      sorter.call a.team, b.team
    end
  end
end
