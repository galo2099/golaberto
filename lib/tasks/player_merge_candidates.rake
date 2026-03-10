require Rails.root.join('lib/player_merge_candidate_finder')

namespace :players do
  desc "Interactively review potential player merges grouped by same country and birth date. Usage: rake players:merge_candidates [THRESHOLD=1.8]"
  task merge_candidates: :environment do
    finder = PlayerMergeCandidateFinder.new
    candidates = finder.find_candidates

    if candidates.empty?
      puts "No candidates found for players with same country and birth date."
      next
    end

    suggested_threshold = finder.suggest_threshold(candidates)
    puts "Found #{candidates.size} candidate pairs."
    puts format("Suggested threshold based on score gap: %.3f", suggested_threshold)

    threshold_input = ENV["THRESHOLD"]
    threshold = threshold_input.present? ? threshold_input.to_f : suggested_threshold

    puts format("Using threshold: %.3f", threshold)
    puts "Commands: [m]erge, [s]kip, [q]uit"

    candidates.each_with_index do |candidate, index|
      break if candidate[:score] < threshold

      left = candidate[:left]
      right = candidate[:right]

      puts "\n## Candidate #{index + 1}"
      puts format("Score: %.3f", candidate[:score])
      puts "Country/Birth: #{left.country} / #{left.birth}"
      print_player_info("A", left, finder)
      print_player_info("B", right, finder)

      print "Decision [m/s/q]: "
      decision = $stdin.gets.to_s.strip.downcase
      decision = "s" if decision.empty?

      if decision == "q"
        puts "Stopping review."
        break
      end

      next if decision != "m"

      print "Merge B into A? [y/N] "
      confirm = $stdin.gets.to_s.strip.downcase
      if confirm == "y"
        left.merge_player(right)
        puts "Merged player #{right.id} into #{left.id}."
      else
        puts "Merge canceled."
      end
    end
  end

  def print_player_info(label, player, finder)
    puts "  Player #{label}: ##{player.id} #{player.name}"
    puts "    Full name: #{player.full_name}" if player.full_name.present?
    puts "    Position: #{player.position || '-'}"

    matches = finder.recent_matches(player)
    if matches.empty?
      puts "    Recent matches: none"
      return
    end

    puts "    Recent matches:"
    matches.each do |match|
      puts "      - #{match[:date]} | #{match[:team_name]} vs #{match[:opponent_name]} | #{match[:score]}"
    end
  end
end
