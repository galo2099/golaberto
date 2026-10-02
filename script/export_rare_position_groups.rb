# Run with: bin/rails runner script/export_rare_position_groups.rb [--include-game-dates] OUTPUT_DIR GROUP_ID...
# Reads the same request fields as Group#odds, without posting or saving odds.
include_game_dates = ARGV.delete("--include-game-dates")
output_dir, *ids = ARGV
abort "usage: bin/rails runner script/export_rare_position_groups.rb [--include-game-dates] OUTPUT_DIR GROUP_ID..." if output_dir.nil? || ids.empty?

require "fileutils"
FileUtils.mkdir_p(output_dir)

ids.each do |raw_id|
  group = Group.includes(phase: :championship, team_groups: :team).find(Integer(raw_id))
  request = OddsRequestBuilder.new(group).build(include_game_dates: include_game_dates)
  games_json = request.fetch("games")
  path = File.join(output_dir, "group-#{group.id}.json")
  File.write(path, JSON.generate(request))
  puts "#{group.id}\t#{group.team_groups.size}\t#{games_json.count { |game| !game['played'] }}\t#{path}"
end
