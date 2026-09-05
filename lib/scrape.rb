# encoding: utf-8

require 'httparty'
require 'fuzzy/fuzzy'

class ChampionshipGet
  include HTTParty

  def self.get(url)
    body = with_http_retries(url)
    ActiveSupport::JSON.decode(body)
  end
end

def with_http_retries(url)
  begin
    ret = ""
    loop do
      ret = HTTParty.get( url, {
        cookies: { 'OptanonConsent': 'isGpcEnabled' },
        headers: {
          "accept": "text/javascript, text/html, application/xml, text/xml, */*",
          "accept-language": "accept-language: en-US,en;q=0.9",
          "cache-control": "no-cache",
          "pragma": "no-cache",
          "priority": "u=1, i",
          "referer": url,
          "user-agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
          "authority" => "www.sofascore.com",
          "accept-encoding" => "deflate, gzip",
          'sec-ch-ua' => '"Not)A;Brand";v="99", "Google Chrome";v="127", "Chromium";v="127"',
          'sec-ch-ua-mobile' => '?0',
          'sec-ch-ua-platform' => '"macOS"',
          'sec-fetch-dest' => 'document',
          'sec-fetch-mode' => 'navigate',
          "connection" => "", },
      }).body

      if not ret.include?("Service Unavailable") and not ret.include?("Internal Server Error")
        break
      end
      Rails.logger.info "Permission error in [#{url}]. Retrying in 1 seconds."
      sleep 1
    end
    return ret
  rescue Errno::ECONNREFUSED, SocketError, Net::ReadTimeout, Net::OpenTimeout
    Rails.logger.info "Cannot reach [#{url}]. Retrying in 1 seconds."
    sleep 1
    retry
  end
end

def fix_name(str)
  if str == "1. FC Köln"
    return "FC Cologne"
  end
  if str == "Dinamo"
    return "Dinamo Tirana"
  end
  if str == "Stade Rennais"
    return "Rennes"
  end
  if str == "Stade Brestois"
    return "Brest"
  end
  if str == "Aris"
    return "Aris Thessaloniki"
  end
  if str == "AVS - Futebol SAD"
    return "AVS"
  end
  if str == "TNS"
    return "The New Saints"
  end
  if str == "TSC"
    return "Bačka Topola"
  end
  if str == "PSG"
    return "Paris Saint-Germain"
  end
  if str == "Atlético Mineiro"
    return "Atlético-MG"
  end
  if str == "Juventude"
    return "Juventude-RS"
  end
  if str == "Atlético Goianiense"
    return "Atlético-GO"
  end
  if str == "Athletico Paranaense"
    return "Atlético-PR"
  end
  if str == "Krasnodar"
    return "FC Krasnodar"
  end
  if str == "Nacional Asunción"
    return "Nacional-PAR"
  end
  if str == "Nacional"
    return "Nacional-URU"
  end
  if str == "Sporting Braga"
    return "Braga"
  end
  if str == "Kardemir Karab&hellip;"
    return "Karabükspor"
  end
  if str == "Akhisarspor"
    return "Akhisar Belediyespor"
  end
  if str == "Rad Beograd"
    return "Rad"
  end
  if str == "Crvena Zvezda"
    return "Red Star Belgrade"
  end
  if str == "FK Crvena zvezda"
    return "Red Star Belgrade"
  end
  if str == "Crvena zvezda"
    return "Red Star Belgrade"
  end
  if str == "Torpedo BelAZ"
    return "Torpedo Zhodino"
  end
  if str == "Zagreb"
    return "NK Zagreb"
  end
  if str == "Split"
    return "RNK Split"
  end
  if str == "Paksi SE"
    return "Paks"
  end
  if str == "Olympique Lyonnais"
    return "Lyon"
  end
  if str == "Ararat"
    return "Ararat Yerevan"
  end
  if str == "Panaitolikos"
    return "Panetolikos"
  end
  if str == "Olympiakos Piraeus"
    return "Olympiakos"
  end
  if str == "FCSB"
    return "Steaua Bucureşti"
  end
  if str == "Universitatea &hellip;"
    return "CS U Craiova"
  end
  if str == "Olympiakos Piraeus"
    return "Olympiakos"
  end
  if str == "Swarovski Tirol"
    return "WSG Wattens"
  end
  if str == "SJ Earthquakes"
    return "San Jose"
  end
  if str == "Republic of Ireland"
    return "Ireland"
  end
  if str == "Rep. Ireland"
    return "Ireland"
  end
  if str == "N. Ireland"
    return "Northern Ireland"
  end
  if str == "CSKA 1948 Sofia"
    return "CSKA 1948"
  end
  if str == "OB"
    return "Odense"
  end
  if str == "København"
    return "Copenhagen"
  end
  if str == "1860 München"
    return "1860 Munich"
  end
  if str == "Inter"
    return "Internazionale"
  end
  if str == "7 de Setembro"
    return "Sete de Dourados"
  end
  if str == "Portuguesa"
    return "Portuguesa-SP"
  end
  if str == "São Raimundo"
    return "São Raimundo-PA"
  end
  if str == "CAP"
    return "Patrocinense-MG"
  end
  if str == "CEOV Operário"
    return "Operário-MT"
  end
  if str == "Guarany de Sobral"
    return "Guarany-CE"
  end
  if str == "Atlético Alagoinhas"
    return "Atlético-BA"
  end
  if str == "Olimpik Sarajevo"
    return "Olimpic"
  end
  if str == "Omonia Nicosia"
    return "Omonia"
  end
  if str == "Floreşti"
    return "Floresti"
  end
  if str == "Qəbələ"
    return "Gabala"
  end
  if str == "Bakı"
    return "Baku"
  end
  if str == "Zhetysu-Sunkar"
    return "Sunkar"
  end
  if str == "Kyzyl-Zhar"
    return "Kyzylzhar"
  end
  if str == "Chinese Taipei"
    return "Taiwan"
  end
  if str == "Korea Republic"
    return "South Korea"
  end
  if str == "Korea Rep"
    return "South Korea"
  end
  if str == "Korea DPR"
    return "North Korea"
  end
  if str == "UAE"
    return "United Arab Emirates"
  end
  if str == "Rīgas FS"
    return "RFS"
  end
  if str == "Rigas FS"
    return "RFS"
  end
  if str == "Žalgiris"
    return "Žalgiris Vilnius"
  end
  if str == "Podgorica"
    return "FK Podgorica"
  end
  if str == "Deportivo Capiatá"
    return "Capiatá"
  end
  if str == "LDU Quito"
    return "LDU"
  end
  if str == "Deportes Tolima"
    return "Tolima"
  end
  if str == "Independiente Santa Fe"
    return "Santa Fe"
  end
  if str == "Petrolero Yacuiba"
    return "Club Petrolero"
  end
  if str == "Deportivo La Guaira"
    return "La Guaira"
  end
  if str == "JBL Zulia"
    return "Deportivo JBL"
  end
  if str == "Unión"
    return "Unión de Santa Fe"
  end
  if str == "Eswatini"
    return "Swaziland"
  end
  if str == "Côte d'Ivoire"
    str = "Ivory Coast"
  end
  if str == "Türkiye"
    str = "Turkey"
  end
  if str == "Czechia"
    str = "Czech Republic"
  end
  if str == "Racing"
    str = "Racing Montevideo"
  end
  if str == "Dinamo Kiev"
    str = "Dynamo Kyiv"
  end
  if str == "Inter de Limeira"
    str = "Internacional-SP"
  end
  if str == "Botafogo"
    str = "Botafogo-RJ"
  end
  if str == "Sounders"
    str = "Seattle"
  end
  if str == "ES Tunis"
    str = "Espérance"
  end
  if str == "Tunis"
    str = "Espérance"
  end
  if str == "LAFC"
    str = "Los Angeles FC"
  end
  if str == "WAC"
    str = "Wolfsberger"
  end
  if str == "USA"
    str = "United States"
  end
  if str == "Montevideo City Torque"
    str = "Torque"
  end
  str
end

def scrape(phase, url, options = {})
  phase = Phase.find phase
  refetch = options[:refetch]

  if url.end_with?('/')
    rounds = nil
    if options[:rounds] then
      rounds = options[:rounds]
    else
      rounds = rounds_to_update(phase)
    end
    altered = false
    rounds.each do |r|
      Rails.logger.info r.inspect
      data = ChampionshipGet.get("#{url}#{r}")
      scrape_events_for_phase(phase, data).each do |match|
        parse_match(phase, data, match, rounds, false, refetch)
      end
    end
  else
    data = ChampionshipGet.get(url)
    scrape_events_for_phase(phase, data).each do |match|
      parse_match(phase, data, match, (1..999999), false, refetch)
    end
  end
end

def scrape_events_for_phase(phase, data)
  events = data["events"] || []
  return events if phase.sofascore_tournament_ids.blank?

  tournament_ids = phase.parsed_sofascore_tournament_ids
  events.select { |match| tournament_ids.include?(match.dig("tournament", "id").to_i) }
end

def rounds_to_update(phase)
  (phase.games.where(date: (Date.today-10.days..Date.today+30.days)).group(:round).size.keys +
   phase.games.where(played: false).where("date < ?", Time.now).map{|g|g.round} +
   phase.games.where(played: true).includes(:player_games).select{|g|g.player_games.size == 0}.map{|g|g.round}).sort.uniq
end

def parse_match(phase, data, match, rounds, create_groups = false, refetch = false)
  status = match["status"]["type"]
  sofascore_id = match["id"]
  round = match.dig("roundInfo", "round")&.to_i

  if status == "postponed"
    postponed_game = phase.games.where(sofascore_id: sofascore_id).first
    if postponed_game and postponed_game.has_time
      postponed_game.has_time = false
      postponed_game.save!
    end
    return
  end

  if status == "canceled"
    return
  end

  fuzzy_match = FuzzyTeamMatch.new
  datetime = Time.at(match["startTimestamp"].to_i).to_datetime
  home_team = match["homeTeam"]
  away_team = match["awayTeam"]
  home_name = fix_name(home_team["name"].gsub(/^\s+/, "").gsub(/\s+$/, ""))
  away_name = fix_name(away_team["name"].gsub(/^\s+/, "").gsub(/\s+$/, ""))

  if create_groups
    Rails.logger.info home_team["country"]["name"]
    Rails.logger.info away_team["country"]["name"]
    home = Team.where(country: fix_country(home_team["country"]["name"])).map{|t| [t, fuzzy_match.getDistance(t.name, home_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
    away = Team.where(country: fix_country(away_team["country"]["name"])).map{|t| [t, fuzzy_match.getDistance(t.name, away_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
    group = phase.groups.select{|g| g.teams.pluck(:id).sort == [ home.id, away.id ].sort }.first
    if group.nil?
      last_group = phase.groups.last.try(:name) || "Group 0"
      tokens = last_group.split(" ")
      tokens[-1].succ!
      new_name = tokens.join " "
      group = phase.groups.build
      group.name = new_name
      group.save!
      group.team_groups << TeamGroup.new(team_id: home.id)
      group.team_groups << TeamGroup.new(team_id: away.id)
    end
  else
    home = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, home_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
    away = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, away_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
  end

  Rails.logger.info "#{home_name} #{home.name}"
  Rails.logger.info "#{away_name} #{away.name}"
  g = nil
  g = phase.games.where(sofascore_id: sofascore_id).includes(:goals, :player_games).first
  unless g
    duplicate_games = phase.games
      .where(home_id: home.id, away_id: away.id, round: round, played: false)
      .includes(:goals, :player_games)
      .order(date: :desc, id: :desc)
      .to_a

    g = duplicate_games.first

    duplicate_games.drop(1).each do |duplicate_game|
      next unless duplicate_game.goals.empty? && duplicate_game.player_games.empty?

      Rails.logger.info "Dropping postponed duplicate game #{duplicate_game.id} for round #{round}"
      duplicate_game.destroy
    end

    g ||= phase.games.where(home_id: home.id, away_id: away.id, round: round, sofascore_id: nil).includes(:goals, :player_games).first
  end
  unless g
    g = phase.games.build({:home_id => home.id, :away_id => away.id})
  end
  if g
    game_compare = g.dup
    game_compare.goals = g.goals
    g.home_id = home.id
    g.away_id = away.id
    g.date = datetime
    g.sofascore_id = sofascore_id
    g.has_time = true
    g.round = round
    g.home_score = 0
    g.away_score = 0
    g.home_aet = nil
    g.away_aet = nil
    g.home_pen = nil
    g.away_pen = nil

    score_data = extract_sofascore_scores(match["homeScore"], match["awayScore"])
    g.home_score = score_data[:full_time_home] || score_data[:final_home] || 0
    g.away_score = score_data[:full_time_away] || score_data[:final_away] || 0
    g.home_aet = score_data[:aet_home]
    g.away_aet = score_data[:aet_away]
    g.home_pen = score_data[:pen_home]
    g.away_pen = score_data[:pen_away]
    g.played = match["status"]["type"] == "finished"
    if g.diff(game_compare).size > 0
      Rails.logger.info g.inspect
      Rails.logger.info game_compare.diff(g).inspect
      g.valid? || raise(g.errors.to_xml.to_s)
      altered = g.save! || altered
    end
    if g.played and rounds.include?(round.to_i) and (refetch or g.player_games.empty?)
      get_scorers(g, "http://www.sofascore.com/api/v1/event/#{sofascore_id}/lineups", "http://www.sofascore.com/api/v1/event/#{sofascore_id}/incidents")
    end
  end
end

def get_scorers(game, lineup_url, incidents_url)
  lineup = ChampionshipGet.get(lineup_url)
  home_lineup = lineup["home"]
  away_lineup = lineup["away"]
  if home_lineup.nil?
    Rails.logger.info "Missing home lineup for #{lineup_url}"
    return
  end
  if away_lineup.nil?
    Rails.logger.info "Missing away lineup for #{lineup_url}"
    return
  end
  incidents = ChampionshipGet.get(incidents_url)
  incidents_list = incidents["incidents"] || []

  game.with_lock do
    has_extra_time_period = incidents_list.any? do |incident|
      incident["incidentType"] == "period" and incident["time"].to_i < 999 and incident_minute(incident) > 90
    end

    if not has_extra_time_period
      game.home_aet = nil
      game.away_aet = nil
      game.save! if game.changed?
    end

    game.goals.clear
    game.player_games.clear
    players = {}
    players_by_name = { 0 => {}, 1 => {} }
    players_by_id = {}
    player_games_by_key = {}
    off = 0
    incidents_list.each do |s|
      if s["incidentType"] == "period" and s["time"] < 999
        minute = incident_minute(s)
        off = [off, minute].max
        off = [120, off].min
      end
    end
    off = 90 if off == 0

    missing_player = false

    proc_player = lambda do |s, game, team_id, end_of_match, is_home|
      name, player_game = process_player(s["player"], game, team_id, end_of_match, !s["substitute"])
      if player_game
        player_game = register_player_game(player_games_by_key, player_game)
        sofascore_id = s.dig("player", "id").to_i
        players[sofascore_id] = player_game if sofascore_id > 0
        players_by_name[is_home ? 0 : 1][name] = player_game
        players_by_id[s["id"]] = player_game if s["id"]
      else
        missing_player = true
      end
    end

    home_lineup["players"].each do |s|
      proc_player.call(s, game, game.home_id, off, true)
    end
    away_lineup["players"].each do |s|
      proc_player.call(s, game, game.away_id, off, false)
    end

    next if players.size == 0

    fuzzy_match = FuzzyTeamMatch.new
    incidents_list.each do |s|
      minute = incident_minute(s)
      if s["incidentType"] == "goal" and s["incidentClass"] == "regular" and not s["player"].nil?
        player = players[s["player"]["id"]]
        if player.nil? and not missing_player
          player = incident_fuzzy_match_player(players_by_name, incident_team_pos(s), s["pl_name"], fuzzy_match)
        end
        Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: false, own_goal: false, aet: incident_extra_time?(s)).save! if player
      end
      if s["incidentType"] == "goal" and s["incidentClass"] == "penalty" and not s["player"].nil?
        player = players[s["player"]["id"]]
        if player.nil? and not missing_player
          player = incident_fuzzy_match_player(players_by_name, incident_team_pos(s), s["pl_name"], fuzzy_match)
        end
        Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: true, own_goal: false, aet: incident_extra_time?(s)).save! if player
      end
      if s["incidentType"] == "goal" and s["incidentClass"] == "ownGoal"
        player = players[s["player"]["id"]]
        if player.nil? and not missing_player
          incident_pos = incident_team_pos(s)
          player = incident_fuzzy_match_player(players_by_name, 1 - incident_pos, s["pl_name"], fuzzy_match) if !incident_pos.nil?
        end
        Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: false, own_goal: true, aet: incident_extra_time?(s)).save! if player
      end
      if s["incidentType"] == "card" and s["incidentClass"] == "yellow"
        # May be a coach
        if not s["player"]
          next
        end
        player = players[s["player"]["id"]]
        if not player
          next
        end
        player.yellow = true
      end
      if s["incidentType"] == "card" and (s["incidentClass"] == "red" || s["incidentClass"] == "yellowRed")
        # May be a coach
        if not s["player"]
          next
        end
        player = players[s["player"]["id"]]
        player.red = true
        player.off = minute if minute < player.off and minute > 0
      end
      if s["incidentType"] == "substitution"
        next if not s["playerIn"] # may be missing
        player = players[s["playerIn"]["id"]]
        next if not player
        player.on = minute
        player.off = off
        player_out_id = incident_player_id(s, "playerOut")
        player_out = player_out_id ? players[player_out_id] : nil
        player_out_name = incident_player_out_name(s)
        if player_out.nil? and player_out_name
          if (player and player_out.nil?) or (player_out.nil? and not missing_player)
            player_out = incident_fuzzy_match_player(players_by_name, incident_team_pos(s), player_out_name, fuzzy_match)
          end
        end
        next if player_out.nil?
        player_out.off = minute
      end
    end
    player_games_by_key.values.each do |p|
      p.save!
    end
  end
end

def register_player_game(player_games_by_key, player_game)
  key = [player_game.game_id, player_game.team_id, player_game.player_id]
  existing_player_game = player_games_by_key[key]
  return player_games_by_key[key] = player_game unless existing_player_game

  existing_player_game.on = [existing_player_game.on.to_i, player_game.on.to_i].min
  existing_player_game.off = [existing_player_game.off.to_i, player_game.off.to_i].max
  existing_player_game.yellow ||= player_game.yellow
  existing_player_game.red ||= player_game.red
  existing_player_game
end

def incident_team_pos(incident)
  pos = incident["pos"]
  return pos if !pos.nil?

  is_home = incident["isHome"]
  return nil if is_home.nil?

  is_home ? 0 : 1
end

def incident_fuzzy_match_player(players_by_name, pos, player_name, fuzzy_match)
  return nil if player_name.nil?

  players_by_name_by_pos = players_by_name[pos]
  return nil if players_by_name_by_pos.nil? || players_by_name_by_pos.empty?

  best_match = players_by_name_by_pos.keys.map{|t| [t, fuzzy_match.getDistance(t, player_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
  Rails.logger.info "#{player_name} - #{best_match}"
  players_by_name_by_pos[best_match]
end

def incident_player_id(incident, key = "player")
  player = incident[key]
  player && player["id"]
end

def incident_player_out_name(incident)
  incident["pl_name_o"] || incident["playerNameOut"]
end

def incident_minute(incident)
  minute = incident["time"]
  if not minute and incident["timeSeconds"]
    minute = incident["timeSeconds"] / 60 + 1
  end
  minute || 0
end

def incident_extra_time?(incident)
  incident_minute(incident) > 90
end

def extract_sofascore_scores(home_score, away_score)
  full_time_home = score_value(home_score, ["normaltime", "NORMAL_TIME", "FINAL_RESULT", "finalResult"])
  full_time_away = score_value(away_score, ["normaltime", "NORMAL_TIME", "FINAL_RESULT", "finalResult"])

  extra_total_home = score_value(home_score, ["afterExtraTime", "extraTime", "EXTRA_TIME", "overtime", "OVER_TIME"])
  extra_total_away = score_value(away_score, ["afterExtraTime", "extraTime", "EXTRA_TIME", "overtime", "OVER_TIME"])

  final_home = score_value(home_score, ["current", "display"])
  final_away = score_value(away_score, ["current", "display"])

  aet_home = extract_aet_goals(home_score, full_time_home, final_home, extra_total_home)
  aet_away = extract_aet_goals(away_score, full_time_away, final_away, extra_total_away)

  {
    full_time_home: full_time_home,
    full_time_away: full_time_away,
    final_home: final_home,
    final_away: final_away,
    aet_home: aet_home,
    aet_away: aet_away,
    pen_home: score_value(home_score, ["penalties", "PENALTIES"]),
    pen_away: score_value(away_score, ["penalties", "PENALTIES"]),
  }
end

def extract_aet_goals(score, full_time, final_score, extra_total)
  period_extra_goals = score_sum(score, ["extra1", "extra2"])
  return period_extra_goals unless period_extra_goals.nil?

  return nil if full_time.nil? || extra_total.nil?

  extra_goal_delta = extra_total - full_time
  return extra_goal_delta if extra_goal_delta > 0

  if !final_score.nil? && full_time + extra_total == final_score
    return extra_total
  end

  [extra_goal_delta, 0].max
end

def score_sum(score, keys)
  return nil unless score

  values = keys.map { |key| score[key] }.compact
  return nil if values.empty?

  values.sum { |value| value.to_i }
end

def score_value(score, keys)
  return nil unless score

  keys.each do |key|
    value = score[key]
    return value.to_i unless value.nil?
  end
  nil
end

def process_player(s, game, team_id, end_of_match, starter)
  sofascore_id = s["id"]
  if sofascore_id.nil?
    return nil, nil
  end
  player = Player.where(sofascore_id: sofascore_id).first
  full_name = s["name"]
  name = s["shortName"]
  birth = Time.at(s["dateOfBirthTimestamp"].to_i).utc.to_datetime
  if not player
#    player = Player.where(full_name: full_name, birth: birth).first
  end
  if not player
#    player = Player.where(full_name: full_name, birth: birth - 1.day).first
  end
  if not player
#    player = Player.where(full_name: full_name, birth: nil).first
  end
  if not player
#    player = Player.where(name: name, birth: birth).first
  end
  if not player
#    player = Player.where(name: name, birth: birth - 1.day).first
  end
  if not player
#    player = Player.where(name: name, birth: nil).first
  end
  if not player
#    player = Player.where("name like '%#{name.to_s.strip.split(/\s+/).last}%'").where(birth: birth).first
  end
  if not player
#    player = Player.where("name like '%#{name.to_s.strip.split(/\s+/).last}%'").where(birth: birth - 1.day).first
  end
  if not player
    player = create_player(s)
  end
  if player.sofascore_id.nil?
    player.sofascore_id = sofascore_id
    player.height = s["height"].to_i if s["height"]
    player.save!
  end
  TeamPlayer.new(team_id: team_id, player_id: player.id, championship_id: game.phase.championship_id).save
  yc = false
  rc = false
  off = 0
  if starter
    off = end_of_match
  end
  on = 0
  return s["name"], PlayerGame.new(player_id: player.id, game_id: game.id, team_id: team_id, on: on, off: off, yellow: yc, red: rc)
end

def create_player(data)
  player = Player.new
  sofascore_id = data["id"]
  name = data["shortName"]
  full_name = data["name"]
  height = data["height"].to_i
  birthday = Time.at(data["dateOfBirthTimestamp"].to_i).utc.to_datetime
  position = data["position"]
  country = fix_country(data["country"]["name"])

  Rails.logger.info name.inspect
  Rails.logger.info sofascore_id.inspect

  player.update(name: name, birth: birthday.to_date, country: country, full_name: full_name, sofascore_id: sofascore_id, height: height)
  if position =~ /G/
    player.position = "g"
  end
  if position =~ /D/
    player.position = "dc"
  end
  if position =~ /M/
    player.position = "cm"
  end
  if position =~ /F/
    player.position = "fw"
  end
  player.save!
  Rails.logger.info player.inspect
  return player
end

def fix_country(c)
  if c == "Andorra CF"
    return "Andorra"
  end
  if c == "Brunei Darussalam"
    return "Brunei"
  end
  if c == "Côte d'Ivoire"
    return "Ivory Coast"
  end
  if c == "Cape Verde Islands"
    return "Cape Verde"
  end
  if c == "Cape Verde Islands"
    return "Cape Verde"
  end
  if c == "Czechia"
    return "Czech Republic"
  end
  if c == "Eswatini"
    return "Swaziland"
  end
  if c == "North Macedonia"
    return "Macedonia"
  end
  if c == "Congo DR"
    return "DR Congo"
  end
  if c == "Hong Kong, China"
    return "Hong Kong"
  end
  if c == "USA"
    return "United States"
  end
  if c == "Republic of Ireland"
    return "Ireland"
  end
  if c == "St. Kitts and Nevis"
    return "Saint Kitts and Nevis"
  end
  if c == "Korea Republic"
    return "South Korea"
  end
  if c == "Korea DPR"
    return "North Korea"
  end
  if c == "Curaçao"
    return "Netherlands Antilles"
  end
  if c == "China PR"
    return "China"
  end
  if c == "St. Lucia"
    return "Saint Lucia"
  end
  if c == "British Virgin Islands"
    return "Virgin Islands (British)"
  end
  if c == "Chinese Taipei"
    return "Taiwan"
  end
  if c == "São Tomé e Príncipe"
    return "Sao Tome and Principe"
  end
  if c == "Kyrgyz Republic"
    return "Kyrgyzstan"
  end
  if c == "Türkiye"
    return "Turkey"
  end
  if c == "Vietnam"
    return "Viet Nam"
  end
  return c
end
