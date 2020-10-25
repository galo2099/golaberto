# encoding: utf-8

require 'httparty'
require 'hpricot'
require 'fuzzy/fuzzy'

class Championship
  include HTTParty

  def self.matches(round_id, competition_id, round = 0)
    date ||= DateTime.now
    data = ActiveSupport::JSON.decode(get( "https://us.soccerway.com/a/block_competition_matches_summary?block_id=page_competition_1_block_competition_matches_summary_6&callback_params=%7B%22page%22%3A34%2C%22block_service_id%22%3A%22competition_summary_block_competitionmatchessummary%22%2C%22round_id%22%3A#{round_id}%2C%22outgroup%22%3Afalse%2C%22view%22%3A1%2C%22competition_id%22%3A#{competition_id}%2C%22bookmaker_urls%22%3A%5B%5D%7D&action=changePage&params=%7B%22page%22%3A#{round-1}%7D", {
      headers: {"User-Agent" => "curl/7.47.0"},
    }).body)
    Hpricot(data["commands"][0]["parameters"]["content"]).search("table/tbody/tr.match")
  end
end

def fix_name(str)
  if str == "Atlético Mineiro"
    return "Atlético-MG"
  end
  if str == "Athletico Paranaense"
    return "Atlético-PR"
  end
  if str == "Krasnodar"
    return "FC Krasnodar"
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
  if str == "Internacional SC"
    return "Inter de Lages-SC"
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
  if str == "UAE"
    return "United Arab Emirates"
  end
  if str == "Rīgas FS"
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
  str
end

def rounds_to_update(phase)
  (phase.games.where(date: (Date.today..Date.today+30.days)).group(:round).size.keys +
   phase.games.where(played: false).where("date < ?", Time.now).map{|g|g.round}).sort.uniq
end

def scrape(phase, round_id, competition_id, options = {})
phase = Phase.find phase
fuzzy_match = FuzzyTeamMatch.new

altered = false
count = 0
round = 0
date = Date.today
rounds = nil
if options[:rounds] then
  rounds = options[:rounds]
else
  rounds = rounds_to_update(phase)
end
rounds.each do |i|
Championship.matches(round_id, competition_id, i).each do |match|
    round = i
    datetime = Time.at(match.attributes["data-timestamp"].to_i).to_datetime.in_time_zone("UTC")
    home_name = fix_name(match.search("td[2]//text()").to_s)
    away_name = fix_name(match.search("td[4]//text()").to_s)
    home = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, home_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
    away = phase.teams.map{|t| [t, fuzzy_match.getDistance(t.name, away_name)]}.sort{|a,b|b[1] <=> a[1]}[0][0]
    p "#{home_name} #{home.name}"
    p "#{away_name} #{away.name}"
    g = phase.games.where(:home_id => home.id, :away_id => away.id, :round => round).first
    unless g
      g = phase.games.build({:home_id => home.id, :away_id => away.id})
    end
    if g
      game_compare = g.dup
      g.date = datetime
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
    end
    count = count + 1
end
end

end
