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
    full_name_matches = finder.full_name_match_candidates(candidates)
    full_name_matches_at_threshold = full_name_matches.count { |candidate| candidate[:score] >= suggested_threshold }
    puts "Found #{candidates.size} candidate pairs."
    puts "Exact full-name matches: #{full_name_matches.size}"
    puts format("Suggested threshold to capture most full-name matches: %.3f", suggested_threshold)
    if full_name_matches.any?
      puts "Full-name matches at/above threshold: #{full_name_matches_at_threshold}/#{full_name_matches.size}"
    end

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

      print "Keep which player? [a/b, default a]: "
      keep_choice = $stdin.gets.to_s.strip.downcase
      keep_choice = "a" if keep_choice.empty?
      if !%w(a b).include?(keep_choice)
        puts "Invalid choice, skipping merge."
        next
      end

      keep_player, remove_player = keep_choice == "a" ? [left, right] : [right, left]
      print "Merge ##{remove_player.id} into ##{keep_player.id}? [y/N] "
      confirm = $stdin.gets.to_s.strip.downcase
      if confirm == "y"
        keep_player.merge_player(remove_player)
        puts "Merged player #{remove_player.id} into #{keep_player.id}."
      else
        puts "Merge canceled."
      end
    end
  end

  def print_player_info(label, player, finder)
    puts "  Player #{label}: ##{player.id} #{player.name}"
    puts "    Full name: #{player.full_name}" if player.full_name.present?
    puts "    Sofascore id: #{player.sofascore_id || '-'}"
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
