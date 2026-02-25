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
      data["events"].each do |match|
        parse_match(phase, data, match, rounds, false, refetch)
      end if data["events"]
    end
  else
    data = ChampionshipGet.get(url)
    data["events"].each do |match|
      parse_match(phase, data, match, (1..999999), false, refetch)
    end if data["events"]
  end
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

    home_score = match["homeScore"]
    away_score = match["awayScore"]
    if home_score
      g.home_score = home_score["display"].to_i
      g.away_score = away_score["display"].to_i
      if home_score["EXTRA_TIME"]
        g.home_aet = home_score["EXTRA_TIME"].to_i - home_score["FINAL_RESULT"].to_i
        g.away_aet = away_score["EXTRA_TIME"].to_i - away_score["FINAL_RESULT"].to_i
      end
      if home_score["penalties"]
        g.home_pen = home_score["penalties"]
        g.away_pen = away_score["penalties"]
      end
    end
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
  game.goals.clear
  game.player_games.clear
  players = {}
  players_by_name = { 0 => {}, 1 => {} }
  players_by_id = {}
  off = 0
  incidents["incidents"].each do |s|
    if s["incidentType"] == "period" and s["time"] < 999
      minute = s["time"]
      if not minute
        minute = s["timeSeconds"] / 60 + 1
      end
      off = [off, minute].max
      off = [120, off].min
    end
  end
  off = 90 if off == 0

  missing_player = false

  proc_player = lambda do |s, game, team_id, end_of_match, is_home|
    name, player = process_player(s["player"], game, team_id, end_of_match, !s["substitute"])
    if player
      players[player.player.sofascore_id.to_i] = player
      players_by_name[is_home ? 0 : 1][name] = player
      players_by_id[s["id"]] = player if s["id"]
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

  if players.size == 0
    return
  end

  fuzzy_match = FuzzyTeamMatch.new
  incidents["incidents"].each do |s|
    minute = s["time"]
    if not minute and s["timeSeconds"]
      minute = s["timeSeconds"] / 60 + 1
    end
    if s["incidentType"] == "goal" and s["incidentClass"] == "regular" and not s["player"].nil?
      player = players[s["player"]["id"]]
      if player.nil? and not missing_player
        best_match = players_by_name[s["pos"]].keys.map{|t| [t, fuzzy_match.getDistance(t, s["pl_name"])]}.sort{|a,b|b[1] <=> a[1]}[0][0]
        player = players_by_name[s["pos"]][best_match]
        Rails.logger.info "#{s["pl_name"]} - #{best_match}"
      end
      Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: false, own_goal: false, aet: minute > 90).save! if player
    end
    if s["incidentType"] == "goal" and s["incidentClass"] == "penalty" and not s["player"].nil?
      player = players[s["player"]["id"]]
      if player.nil? and not missing_player
        best_match = players_by_name[s["pos"]].keys.map{|t| [t, fuzzy_match.getDistance(t, s["pl_name"])]}.sort{|a,b|b[1] <=> a[1]}[0][0]
        player = players_by_name[s["pos"]][best_match]
        Rails.logger.info "#{s["pl_name"]} - #{best_match}"
      end
      Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: true, own_goal: false, aet: minute > 90).save! if player
    end
    if s["incidentType"] == "goal" and s["incidentClass"] == "ownGoal"
      player = players[s["player"]["id"]]
      if player.nil? and not missing_player
        best_match = players_by_name[1 - s["pos"]].keys.map{|t| [t, fuzzy_match.getDistance(t, s["pl_name"])]}.sort{|a,b|b[1] <=> a[1]}[0][0]
        player = players_by_name[1 - s["pos"]][best_match]
        Rails.logger.info "#{s["pl_name"]} - #{best_match}"
      end
      Goal.new(player_id: player.player_id, game_id: game.id, team_id: player.team_id, time: minute, penalty: false, own_goal: true, aet: minute > 90).save! if player
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
      player_out = players[s["playerOut"]["id"]]
      if (player and player_out.nil?) or (player_out.nil? and not missing_player)
        best_match = players_by_name[s["pos"]].keys.map{|t| [t, fuzzy_match.getDistance(t, s["pl_name_o"])]}.sort{|a,b|b[1] <=> a[1]}[0][0]
        player_out = players_by_name[s["pos"]][best_match]
        Rails.logger.info "#{s["pl_name_o"]} - #{best_match}"
      end
      player_out.off = minute
    end
  end
  players.values.each do |p|
    p.save!
  end
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
