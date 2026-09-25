# Run with: bin/rails runner script/export_rare_position_groups.rb OUTPUT_DIR GROUP_ID...
# Reads the same request fields as Group#odds, without posting or saving odds.
output_dir, *ids = ARGV
abort "usage: bin/rails runner script/export_rare_position_groups.rb OUTPUT_DIR GROUP_ID..." if output_dir.nil? || ids.empty?

require "fileutils"
FileUtils.mkdir_p(output_dir)

ids.each do |raw_id|
  group = Group.includes(phase: :championship, team_groups: :team).find(Integer(raw_id))
  games_json = group.games.includes(:home, :away).as_json(
    methods: [:home_power, :away_power],
    only: [:id, :home_id, :away_id, :home_score, :away_score, :played]
  )
  request = group.as_json(
    include: {
      phase: { include: :championship },
      team_groups: { only: [:team_id, :add_sub, :bias] }
    }
  ).merge("games" => games_json)
  path = File.join(output_dir, "group-#{group.id}.json")
  File.write(path, JSON.generate(request))
  puts "#{group.id}\t#{group.team_groups.size}\t#{games_json.count { |game| !game['played'] }}\t#{path}"
end
