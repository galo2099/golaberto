# Run with: bin/rails runner script/select_rare_position_benchmark_groups.rb OUTPUT_JSON
# Read-only selection of varied 20-team groups for the offline benchmark.
output_path = ARGV.fetch(0)
ids = ActiveRecord::Base.connection.select_values(<<~SQL)
  SELECT group_id FROM team_groups GROUP BY group_id
  HAVING COUNT(*) = 20 ORDER BY group_id DESC LIMIT 100
SQL

rows = ids.map do |id|
  group = Group.includes(phase: :championship, team_groups: :team).find(id)
  teams = group.team_groups.map(&:team_id)
  points = group.team_groups.to_h { |tg| [tg.team_id, tg.add_sub.to_i] }
  games = group.games.to_a
  rules = group.phase.championship
  games.each do |game|
    next unless game.played && game.home_score && game.away_score
    home, away = game.home_score, game.away_score
    points[game.home_id] += home > away ? rules.point_win : home == away ? rules.point_draw : rules.point_loss if points.key?(game.home_id)
    points[game.away_id] += away > home ? rules.point_win : home == away ? rules.point_draw : rules.point_loss if points.key?(game.away_id)
  end
  sorted = teams.map { |team| points[team] }.sort.reverse
  {
    group: group.id,
    unplayed: games.count { |game| !game.played },
    played: games.count(&:played),
    points_spread: sorted.first - sorted.last,
    top_gap: sorted[0] - sorted[1],
    bottom_gap: sorted[-2] - sorted[-1]
  }
end

eligible = rows.select { |row| row[:unplayed].between?(80, 360) }
abort "no eligible 20-team groups" if eligible.empty?
selected = [16982]
criteria = {
  tight: ->(row) { [row[:points_spread], -row[:unplayed]] },
  wide: ->(row) { [-row[:points_spread], -row[:unplayed]] },
  dominant: ->(row) { [-row[:top_gap], -row[:points_spread]] },
  weak: ->(row) { [-row[:bottom_gap], -row[:points_spread]] },
  many_unplayed: ->(row) { [-row[:unplayed], row[:points_spread]] },
  few_unplayed: ->(row) { [row[:unplayed], row[:points_spread]] }
}
labels = { baseline: 16982 }
criteria.each do |label, key|
  winner = eligible.reject { |row| selected.include?(row[:group]) }.min_by(&key)
  next unless winner
  selected << winner[:group]
  labels[label] = winner[:group]
end
File.write(output_path, JSON.pretty_generate({selected: labels, candidates: rows}) + "\n")
puts labels.map { |label, id| "#{label}\t#{id}" }
