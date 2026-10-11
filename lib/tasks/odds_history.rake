namespace :odds_history do
  desc "Backfill daily odds history for championship groups. Usage: rake odds_history:backfill CHAMPIONSHIP_ID=1 [PHASE_ID=2] [GROUP_ID=3] [FROM=2024-01-01] [TO=2024-12-31] [RESET=true]"
  task backfill: :environment do
    championship_id = ENV["CHAMPIONSHIP_ID"]
    phase_id = ENV["PHASE_ID"]
    group_id = ENV["GROUP_ID"]
    from_date = ENV["FROM"].present? ? Date.parse(ENV["FROM"]) : nil
    to_date = ENV["TO"].present? ? Date.parse(ENV["TO"]) : nil
    reset = ENV["RESET"].to_s == "true"

    raise "CHAMPIONSHIP_ID is required" if championship_id.blank?

    OddsHistoryBackfillService.run(
      championship_id: championship_id,
      phase_id: phase_id,
      group_id: group_id,
      from_date: from_date,
      to_date: to_date,
      reset: reset,
    )
  end
end
