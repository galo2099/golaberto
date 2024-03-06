# encoding: utf-8

require 'httparty'
require 'hpricot'
require 'fuzzy/fuzzy'

class ChampionshipGet
  include HTTParty

  def self.matches(round_id, competition_id, round = 0)
    date ||= DateTime.now
    data = with_http_retries { ActiveSupport::JSON.decode(get( "https://us.soccerway.com/a/block_competition_matches_summary?block_id=page_competition_1_block_competition_matches_summary_6&callback_params=%7B%22page%22%3A34%2C%22block_service_id%22%3A%22competition_summary_block_competitionmatchessummary%22%2C%22round_id%22%3A#{round_id}%2C%22outgroup%22%3Afalse%2C%22view%22%3A1%2C%22competition_id%22%3A#{competition_id}%2C%22bookmaker_urls%22%3A%5B%5D%7D&action=changePage&params=%7B%22page%22%3A#{round-1}%7D", {
      headers: {"user-agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36",
                "authority" => "us.soccerway.com",
                "accept" => "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9",
                "accept-language" => "en-US,en;q=0.9,pt-BR;q=0.8,pt;q=0.7,es;q=0.6",
                "accept-encoding" => "deflate, gzip",
                "connection" => "", },
    }).body) }
    Hpricot(data["commands"][0]["parameters"]["content"]).search("table/tbody/tr.match")
  end
end

def with_http_retries(&block)
  begin
    yield
  rescue Errno::ECONNREFUSED, SocketError, Net::ReadTimeout, Net::OpenTimeout
    p "Cannot reach [#{@service_url}]. Retrying in #{@retry_seconds} seconds."
    sleep 1
    retry
  end
end

def fix_name(str)
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
  if str == "Inter Milan"
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
    return "Operário Várzea-Grandense-MS"
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
  str
end

def rounds_to_update(phase)
  (phase.games.where(date: (Date.today-10.days..Date.today+30.days)).group(:round).size.keys +
   phase.games.where(played: false).where("date < ?", Time.now).map{|g|g.round} +
   phase.games.where(played: true).includes(:player_games).select{|g|g.player_games.size == 0}.map{|g|g.round}).sort.uniq
end

def scrape(phase, round_id, competition_id, options = {})
  phase = Phase.find phase

  altered = false
  round = 0
  date = Date.today
  rounds = nil
  if options[:rounds] then
    rounds = options[:rounds]
  else
    rounds = rounds_to_update(phase)
  end
  rounds.each do |i|
    puts "Round #{i}"
    ChampionshipGet.matches(round_id, competition_id, i).each do |match|
      parse_match(phase, i, match)
    end
  end
end

def parse_match(phase, round, match)
  fuzzy_match = FuzzyTeamMatch.new
  datetime = Time.at(match.attributes["data-timestamp"].to_i).to_datetime.in_time_zone("UTC")
  home_name = fix_name(match.search("td[2]//text()").to_s.gsub(/^\s+/, "").gsub(/\s+$/, ""))
  away_name = fix_name(match.search("td[4]//text()").to_s.gsub(/^\s+/, "").gsub(/\s+$/, ""))
  home = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, home_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
  away = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, away_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
  p "#{home_name} #{home.name}"
  p "#{away_name} #{away.name}"
  g = nil
  soccerway_url = match.search("td[3]/a").first.attributes["href"]
  soccerway_id = soccerway_url.gsub(/.*?(\d+).$/, '\1')
  g = phase.games.where(soccerway_id: soccerway_id).includes(:goals).first
  # legacy
  unless g
    g = phase.games.where(home_id: home.id, away_id: away.id, round: round, soccerway_id: nil).includes(:goals).first
  end
  unless g
    g = phase.games.build({:home_id => home.id, :away_id => away.id})
  end
  if g
    game_compare = g.dup
    game_compare.goals = g.goals
    g.date = datetime
    g.soccerway_id = soccerway_id
    g.has_time = true
    g.round = round
    g.played = "0"
    if match.search("td[3]//text()").to_s =~ /PSTP/
      g.has_time = false
    end
    if match.search("td[3]//text()").to_s =~ /(\d+) - (\d+)/
      g.home_score = $1.to_i
      g.away_score = $2.to_i
      g.played = true
    end
    if match.search("td[1]//text()").to_s !~ /FT/
      g.played = false
    end
    if g.diff(game_compare).size > 0
      p g
      p game_compare.diff(g)
      g.valid? || raise(g.errors.to_xml.to_s)
      altered = g.save! || altered
    end
    if g.played
      get_scorers(g, "https://us.soccerway.com" + match.search("td[3]/a").first.attributes["href"])
    end
  end
end

def get_scorers(game, url)
  body = with_http_retries { HTTParty.get(url, { headers: {"user-agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36",
                "authority" => "us.soccerway.com",
                "accept" => "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9",
                "accept-language" => "en-US,en;q=0.9,pt-BR;q=0.8,pt;q=0.7,es;q=0.6",
                "accept-encoding" => "deflate, gzip",
                "connection" => "", } }).body }
  data = Hpricot(body)
  game.goals.clear
  game.player_games.clear
  players = {}
  if not data.search('//*[@id="yui-main"]//div[@class="combined-lineups-container"]').first
    return
  end
  state = data.search('//*[@id="page_match_1_block_match_info_5"]//span.match-state/text()').first.to_s
  off = 90
  if state == "AET"
    off = 120
  end
  data.search('//*[@id="yui-main"]//div[@class="combined-lineups-container"]')[0].search('/div[1]/table/tbody/tr').each do |s|
    player = process_player(s, game, game.home_id, players, true, off)
    players[player.player.soccerway_id] = player if player
  end
  data.search('//*[@id="yui-main"]//div[@class="combined-lineups-container"]')[0].search('/div[2]/table/tbody/tr').each do |s|
    player = process_player(s, game, game.away_id, players, true, off)
    players[player.player.soccerway_id] = player if player
  end
  data.search('//*[@id="yui-main"]//div[@class="combined-lineups-container"]')[1].search('/div[2]/table/tbody/tr').each do |s|
    player = process_player(s, game, game.home_id, players, false, off)
    players[player.player.soccerway_id] = player if player
  end
  data.search('//*[@id="yui-main"]//div[@class="combined-lineups-container"]')[1].search('/div[3]/table/tbody/tr').each do |s|
    player = process_player(s, game, game.away_id, players, false, off)
    players[player.player.soccerway_id] = player if player
  end
  players.values.each do |p|
    p.save!
  end
end

def process_player(s, game, team_id, players, starter, end_of_match)
  link = s.search('/td.player.large-link//a[1]').first
  if not link
    return
  end
  soccerway_url = link.attributes['href']
  soccerway_id = soccerway_url.gsub(/.*?(\d+).$/, '\1')
  player = Player.where(soccerway_id: soccerway_id).first
  if not player
    player = create_player("https://us.soccerway.com" + soccerway_url, soccerway_id)
  end
  TeamPlayer.new(team_id: team_id, player_id: player.id, championship_id: game.phase.championship_id).save
  yc = false
  rc = false
  off = 0
  if starter
    off = end_of_match
  end
  out = s.search('/td.player.large-link/p.substitute-out').first
  on = 0
  if out
    out_id = out.search('/a').first.attributes['href'].gsub(/.*?(\d+).$/, '\1')
    minute = out.search('/text()').to_s.gsub(/.*?(\d+).*/m, '\1')
    if players[out_id]
      players[out_id].off = minute
    end
    on = minute
    off = end_of_match
  end
  s.search('/td.bookings/span').each do |span|
    if span.search('/img').first.attributes['src'] =~ /\bG.png\b/
      minute = span.search('text()').first.to_s.gsub(/.*?(\d+).*/m, '\1').to_i
      Goal.new(player_id: player.id, game_id: game.id, team_id: team_id, time: minute, penalty: false, own_goal: false, aet: minute > 90).save!
    end
    if span.search('/img').first.attributes['src'] =~ /\bPG.png\b/
      minute = span.search('text()').first.to_s.gsub(/.*?(\d+).*/m, '\1').to_i
      Goal.new(player_id: player.id, game_id: game.id, team_id: team_id, time: minute, penalty: true, own_goal: false, aet: minute > 90).save!
    end
    if span.search('/img').first.attributes['src'] =~ /\bOG.png\b/
      minute = span.search('text()').first.to_s.gsub(/.*?(\d+).*/m, '\1').to_i
      Goal.new(player_id: player.id, game_id: game.id, team_id: team_id, time: minute, penalty: false, own_goal: true, aet: minute > 90).save!
    end
    if span.search('/img').first.attributes['src'] =~ /\bYC.png\b/
      yc = true
    end
    if span.search('/img').first.attributes['src'] =~ /\bRC.png\b/
      rc = true
      off = span.search('text()').first.to_s.gsub(/.*?(\d+).*/m, '\1') if off != 0
    end
    if span.search('/img').first.attributes['src'] =~ /\bY2C.png\b/
      rc = true
      off = span.search('text()').first.to_s.gsub(/.*?(\d+).*/m, '\1') if off != 0
    end
  end
  return PlayerGame.new(player_id: player.id, game_id: game.id, team_id: team_id, on: on, off: off, yellow: yc, red: rc)
end

def create_player(url, soccerway_id)
  data = with_http_retries { HTTParty.get(url, { headers: {"user-agent" => "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36",
                "authority" => "us.soccerway.com",
                "accept" => "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.9",
                "accept-language" => "en-US,en;q=0.9,pt-BR;q=0.8,pt;q=0.7,es;q=0.6",
                "accept-encoding" => "deflate, gzip",
                "connection" => "", }}).body }
  player_info = Hpricot(data)
  name = player_info.search('//*[@id="subheading"]/h1//text()').to_s
  full_name = player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-first_name="first_name"]//text()').to_s + " " + player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-last_name="last_name"]//text()').to_s
  height = player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-height="height"]//text()').to_s
  birthday = player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-date_of_birth="date_of_birth"]//text()').to_s
  position = player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-position="position"]//text()').to_s
  country = player_info.search('//*[@id="page_player_1_block_player_passport_3"]/div/div/div[1]/div/dl/dd[@data-nationality="nationality"]//text()').to_s

  p name
  p soccerway_id

  if country == "Brunei Darussalam"
    country = "Brunei"
  end
  if country == "Côte d'Ivoire"
    country = "Ivory Coast"
  end
  if country == "Cape Verde Islands"
    country = "Cape Verde"
  end
  if country == "Cape Verde Islands"
    country = "Cape Verde"
  end
  if country == "Czechia"
    country = "Czech Republic"
  end
  if country == "Eswatini"
    country = "Swaziland"
  end
  if country == "North Macedonia"
    country = "Macedonia"
  end
  if country == "Congo DR"
    country = "DR Congo"
  end
  if country == "Hong Kong, China"
    country = "Hong Kong"
  end
  if country == "USA"
    country = "United States"
  end
  if country == "Republic of Ireland"
    country = "Ireland"
  end
  if country == "St. Kitts and Nevis"
    country = "Saint Kitts and Nevis"
  end
  if country == "Korea Republic"
    country = "South Korea"
  end
  if country == "Korea DPR"
    country = "North Korea"
  end
  if country == "Curaçao"
    country = "Netherlands Antilles"
  end
  if country == "China PR"
    country = "China"
  end
  if country == "St. Lucia"
    country = "Saint Lucia"
  end
  if country == "British Virgin Islands"
    country = "Virgin Islands (British)"
  end
  if country == "Chinese Taipei"
    country = "Taiwan"
  end
  if country == "São Tomé e Príncipe"
    country = "Sao Tome and Principe"
  end
  if country == "Kyrgyz Republic"
    country = "Kyrgyzstan"
  end
  if country == "Türkiye"
    country = "Turkey"
  end
  if country == "Vietnam"
    country = "Viet Nam"
  end

  player = Player.new(name: name, birth: birthday.to_date, country: country, full_name: full_name, soccerway_id: soccerway_id, height: height.try(:to_i))
  if position =~ /Goalkeeper/
    player.position = "g"
  end
  if position =~ /Defender/
    player.position = "dc"
  end
  if position =~ /Midfielder/
    player.position = "cm"
  end
  if position =~ /Attacker/
    player.position = "fw"
  end
  player.save!
  puts player.name
  return player
end
