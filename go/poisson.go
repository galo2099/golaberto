package main

import "github.com/dgryski/go-metro"

import _ "net/http/pprof"
import "fmt"
import "net/http"
import "encoding/binary"
import "encoding/json"
import "log"
import "math"
import "math/rand"
import "sort"
import "strconv"
import "strings"
import "time"

type TeamCampaign struct {
  id int
  points int
  wins int
  draws int
  losses int
  goals_for int
  goals_against int
  goals_away int
  bias int
  add_sub int
  points_win int
  points_draw int
  points_loss int
  home_games []*GameType
  uses_head bool
}

func (campaign *TeamCampaign) goals_diff() int {
  return campaign.goals_for - campaign.goals_against
}

func (campaign *TeamCampaign) goals_average() float64 {
  return float64(campaign.goals_for) / (float64(campaign.goals_against) + 0.0000000000001)
}

func (campaign *TeamCampaign) clone() *TeamCampaign {
  t := new(TeamCampaign)
  t.id = campaign.id
  t.points = campaign.points
  t.wins = campaign.wins
  t.draws = campaign.draws
  t.losses = campaign.losses
  t.goals_for = campaign.goals_for
  t.goals_against = campaign.goals_against
  t.goals_away = campaign.goals_away
  t.bias = campaign.bias
  t.add_sub = campaign.add_sub
  t.points_win = campaign.points_win
  t.points_draw = campaign.points_draw
  t.points_loss = campaign.points_loss
  t.uses_head = campaign.uses_head
  if t.uses_head {
    t.home_games = make([]*GameType, len(campaign.home_games))
    copy(t.home_games, campaign.home_games)
  }
  return t
}

func (campaign *TeamCampaign) add_game(game *GameType) {
  is_home := campaign.id == game.Home_id
  if game.Home_score > game.Away_score {
    if is_home {
      campaign.wins += 1
      campaign.points += campaign.points_win
    } else {
      campaign.losses += 1
      campaign.points += campaign.points_loss
    }
  } else if game.Home_score < game.Away_score {
    if is_home {
      campaign.losses += 1
      campaign.points += campaign.points_loss
    } else {
      campaign.wins += 1
      campaign.points += campaign.points_win
    }
  } else {
    campaign.draws += 1
    campaign.points += campaign.points_draw
  }
  if is_home {
    if campaign.uses_head {
      campaign.home_games = append(campaign.home_games, game)
    }
    campaign.goals_for += game.Home_score
    campaign.goals_against += game.Away_score
  } else {
    campaign.goals_for += game.Away_score
    campaign.goals_against += game.Home_score
    campaign.goals_away += game.Away_score
  }
}

type GameType struct {
  Id int
  Home_id int
  Away_id int
  Home_score int
  Away_score int
  Home_power float64
  Away_power float64
  Played bool
}

type PhaseType struct {
  Championship *ChampionshipType
  Bonus_points int
  Bonus_points_threshold int
  Sort string
}

type ChampionshipType struct {
  Point_draw int
  Point_loss int
  Point_win int
}

type TeamType struct {
  Team_id int
  Add_sub int
  Bias int
}

type ZoneType struct {
  Position []int
}

type GroupType struct {
  Zones []ZoneType
  Phase *PhaseType
  Games []GameType
  Team_groups []TeamType
}

func factorial(x int) int {
  ret := 1
  for i := 2; i <= x; i++  {
    ret *= i
  }
  return ret
}

func p(mean float64, x int) float64 {
  return math.Pow(mean, float64(x)) * math.Exp(-mean) / float64(factorial(x))
}

func poisson_rand(mean float64) int {
  die := rand.Float64()
  i := 0
  accumlated_probability := p(mean, i)
  for die > accumlated_probability {
    i += 1
    accumlated_probability += p(mean, i)
  }
  return i
}

type SortType int
const (
  RANDOM SortType = iota
  PT
  W
  GD
  GF
  G_AVERAGE
  G_AWAY
  BIAS
  G_AET
  GP
  HEAD
)

type TeamCampaignSorted struct {
  t []*TeamCampaign
  sort []SortType
}

func (sorted TeamCampaignSorted) Len() int {
  return len(sorted.t)
}

func (sorted TeamCampaignSorted) Swap(i, j int) {
  sorted.t[i], sorted.t[j] = sorted.t[j], sorted.t[i]
}

func (sorted TeamCampaignSorted) Less(i, j int) bool {
  for _, sort := range sorted.sort {
    var a, b float64
    switch sort {
      case PT:
        a, b = float64(sorted.t[i].points), float64(sorted.t[j].points)
      case W:
        a, b = float64(sorted.t[i].wins), float64(sorted.t[j].wins)
      case GD:
        a, b = float64(sorted.t[i].goals_diff()), float64(sorted.t[j].goals_diff())
      case GF:
        a, b = float64(sorted.t[i].goals_for), float64(sorted.t[j].goals_for)
      case G_AVERAGE:
        a, b = float64(sorted.t[i].goals_average()), float64(sorted.t[j].goals_average())
      case G_AWAY:
        a, b = float64(sorted.t[i].goals_away), float64(sorted.t[j].goals_away)
      case BIAS:
        a, b = float64(sorted.t[i].bias), float64(sorted.t[j].bias)
      case G_AET:
        a, b = 0, 0
      case GP:
        a, b = 0, 0
      case HEAD:
        ret := compare_head_to_head(sorted.t[i], sorted.t[j], sorted.sort)
        if ret < 0 {
          a, b = 0, 1
        } else if ret > 0 {
          a, b = 1, 0
        } else {
          a, b = 0, 0
        }
      default:
        return rand.Float64() < 0.5
    }
    if b < a {
      return true
    } else if b > a {
      return false
    }
  }
  return false
}

func build_sorted_array(s []string) []SortType {
  ret := make([]SortType, len(s))
  for i := range s {
    switch s[i] {
       case "pt":
         ret[i] = PT
       case "w":
         ret[i] = W
       case "gd":
         ret[i] = GD
       case "gf":
         ret[i] = GF
       case "g_average":
         ret[i] = G_AVERAGE
       case "g_away":
         ret[i] = G_AWAY
       case "bias":
         ret[i] = BIAS
       case "g_aet":
         ret[i] = G_AET
       case "gp":
         ret[i] = GP
       case "head":
         ret[i] = HEAD
       default:
         ret[i] = RANDOM
    }
  }
  return ret
}

func compare_head_to_head(a, b *TeamCampaign, sort []SortType) int {
  head_to_head_campaign := make([]*TeamCampaign, 2)
  head_to_head_campaign[0] = &TeamCampaign{id: a.id,
      points_win: a.points_win,
      points_draw: a.points_draw,
      points_loss: a.points_loss,
      uses_head: false}
  head_to_head_campaign[1] = &TeamCampaign{id: b.id,
      points_win: a.points_win,
      points_draw: a.points_draw,
      points_loss: a.points_loss,
      uses_head: false}
  for i := range a.home_games {
    if (a.home_games[i].Away_id == b.id) {
      head_to_head_campaign[0].add_game(a.home_games[i])
      head_to_head_campaign[1].add_game(a.home_games[i])
    }
  }
  for i := range b.home_games {
    if (b.home_games[i].Away_id == a.id) {
      head_to_head_campaign[0].add_game(b.home_games[i])
      head_to_head_campaign[1].add_game(b.home_games[i])
    }
  }
  sort_no_head := make([]SortType, 0, len(sort))
  for _, s := range sort {
    if s != RANDOM && s != HEAD {
      sort_no_head = append(sort_no_head, s)
    }
  }
  sorted_teams := TeamCampaignSorted{head_to_head_campaign, sort_no_head}
  ret := 0
  if sorted_teams.Less(1, 0) {
    ret = -1
  } else if sorted_teams.Less(0, 1) {
    ret = 1
  }
  return ret
}

type TeamOdds struct {
  count int
  Pos []float64
}

type SimulatedScore struct {
  home int
  away int
}

type GameOdds struct {
  game_index int
  scores []*TeamOdds
}

func make_unitary(x []float64) []float64 {
  ret := make([]float64, len(x))
  for i := range x {
    ret[i] = math.Pow(x[i] / 100, 0.5)
  }
  return ret
}

func (group *GroupType) odds_to_zone_odds(odds []float64) []float64 {
  zone_odds := make([]float64, len(group.Zones) + 1)
  for i, v := range odds {
    var found bool
    for j, zone := range group.Zones {
      for _, p := range zone.Position {
        if i + 1 == p {
          zone_odds[j] += v
          found = true
        }
      }
    }
    if !found {
      zone_odds[len(zone_odds)-1] += v
    }
  }
  return zone_odds
}

func make_index_from_simulated_score(score SimulatedScore) int {
  if score.home > 9 {
    score.home = 9
  }
  if score.away > 9 {
    score.away = 9
  }
  return score.home * 10 + score.away
}

type OddsType struct {
  team *TeamOdds
  games []*GameOdds
}

func (group *GroupType) calculate_odds() map[string]interface{} {
  start_func := time.Now()
  sort_order := build_sorted_array(strings.Split(strings.Join(strings.Fields(group.Phase.Sort), ""), ","))
  uses_head := false
  for _, v := range sort_order {
    if v == HEAD {
      uses_head = true
    }
  }
  campaign := make([]*TeamCampaign, len(group.Team_groups))

  all_team_ids := make([]uint32, len(group.Team_groups))
  for i, t := range group.Team_groups {
    all_team_ids[i] = uint32(t.Team_id)
  }
  table := NewTable(all_team_ids)

  for _, t := range group.Team_groups {
    campaign[table.Query(uint32(t.Team_id))] = &TeamCampaign{id: t.Team_id,
      points: t.Add_sub,
      bias: t.Bias,
      add_sub: t.Add_sub,
      points_win: group.Phase.Championship.Point_win,
      points_draw: group.Phase.Championship.Point_draw,
      points_loss: group.Phase.Championship.Point_loss,
      uses_head: uses_head}
  }

  for _, g := range group.Games {
    if g.Played {
      home := table.Query(uint32(g.Home_id))
      away := table.Query(uint32(g.Away_id))
      if campaign[home] != nil {
        campaign[home].add_game(&g)
      }
      if campaign[away] != nil {
        campaign[away].add_game(&g)
      }
    }
  }

  team_odds := make([]OddsType, len(group.Team_groups))
  for k, _ := range group.Team_groups {
    team_odds[k] = OddsType{&TeamOdds{0, make([]float64, len(campaign))}, make([]*GameOdds, 0)}
  }

  for i, g := range group.Games {
    if !g.Played {
      home := table.Query(uint32(g.Home_id))
      away := table.Query(uint32(g.Away_id))
      team_odds[home].games = append(team_odds[home].games, &GameOdds{i, make([]*TeamOdds, 100)})
      team_odds[away].games = append(team_odds[away].games, &GameOdds{i, make([]*TeamOdds, 100)})
    }
  }

  var elapsed time.Duration
  simulated_scores := make([]SimulatedScore, len(group.Games))
  odds_rest := make([]float64, len(campaign))
  simulated_campaign := make([]*TeamCampaign, len(campaign))
  team_slice := make([]*TeamCampaign, len(campaign));
  const NUM_ITER = 10000
  for i := 0; i < NUM_ITER; i++ {
    for k, v := range campaign {
      simulated_campaign[k] = v.clone()
    }

    for i, g := range group.Games {
      if !g.Played {
        home_score := poisson_rand(g.Home_power)
        away_score := poisson_rand(g.Away_power)
        simulated_scores[i] = SimulatedScore{home_score, away_score}
        home := table.Query(uint32(g.Home_id))
        away := table.Query(uint32(g.Away_id))
        if simulated_campaign[home] != nil {
          simulated_campaign[home].add_game(&GameType{g.Id, g.Home_id, g.Away_id, home_score, away_score, 0.0, 0.0, true})
        }
        if simulated_campaign[away] != nil {
          simulated_campaign[away].add_game(&GameType{g.Id, g.Home_id, g.Away_id, home_score, away_score, 0.0, 0.0, true})
        }
      }
    }

    {
      i := 0
      for _, v := range simulated_campaign {
        team_slice[i] = v
        i += 1
      }
    }

    sorted_teams := TeamCampaignSorted{team_slice, sort_order}
    sort.Sort(sorted_teams)

    start := time.Now()
    for i, v := range sorted_teams.t {
      team := team_odds[table.Query(uint32(v.id))]
      team.team.Pos[i] += 1.0
      for k, g := range team.games {
        odds := g.scores[make_index_from_simulated_score(simulated_scores[g.game_index])]
        if odds == nil {
          odds = &TeamOdds{0, make([]float64, len(sorted_teams.t))}
          team.games[k].scores[make_index_from_simulated_score(simulated_scores[g.game_index])] = odds
        }
        odds.count += 1
        odds.Pos[i] += 1.0
      }
    }
    elapsed += time.Since(start)
  }  // end for

  result := make(map[string]interface{})
  json_team_odds := make(map[int]*TeamOdds, len(team_odds))
  for _, team := range campaign {
    index := table.Query(uint32(team.id))
    for pos := range team_odds[index].team.Pos {
      team_odds[index].team.Pos[pos] /= NUM_ITER / 100
    }
    json_team_odds[team.id] = team_odds[index].team
  }
  result["team_odds"] = json_team_odds
  game_importances := make(map[int]float64)
  home_importances := make(map[int]float64)
  away_importances := make(map[int]float64)
  start2 := time.Now()
  for i, g := range group.Games {
    if !g.Played {
      home := table.Query(uint32(g.Home_id))
      away := table.Query(uint32(g.Away_id))
      var home_importance float64
      for _, x := range team_odds[home].games {
        if x.game_index != i {
          continue;
        }
        for k, v := range x.scores {
          if v == nil {
            continue
          }
          for j, _ := range v.Pos {
            v.Pos[j] /= float64(v.count) / 100
          }
          odds := team_odds[home].team
          for i, o := range odds.Pos {
            odds_rest[i] = (o * NUM_ITER / 100 - v.Pos[i] * float64(v.count) / 100) / (float64(NUM_ITER) - float64(v.count)) * 100
            if odds_rest[i] < 0 {
              odds_rest[i] = 0
            }
          }
          //distance := calculateEuclidianDistance(group.odds_to_zone_odds(odds_rest), group.odds_to_zone_odds(v.Pos))
          distance := calculateSimilarity(group.odds_to_zone_odds(odds_rest), group.odds_to_zone_odds(v.Pos))
          if g.Id == 273785 {
            log.Printf("%d %d %v %v %v\n", g.Id, g.Home_id, k, v.count, group.odds_to_zone_odds(v.Pos))
            log.Printf("%d %d %v %v\n", g.Id, g.Home_id, NUM_ITER - v.count, group.odds_to_zone_odds(odds_rest))
            log.Printf("distance %v\n", distance)
          }
          home_importance += (distance) * float64(v.count) / NUM_ITER
        }
      }
      var away_importance float64
      for _, x := range team_odds[away].games {
        if x.game_index != i {
          continue;
        }
        for k, v := range x.scores {
          if v == nil {
            continue
          }
          for j, _ := range v.Pos {
            v.Pos[j] /= float64(v.count) / 100
          }
          odds := team_odds[away].team
          odds_rest := make([]float64, len(v.Pos))
          for i, o := range odds.Pos {
            odds_rest[i] = (o * NUM_ITER / 100 - v.Pos[i] * float64(v.count) / 100) / (float64(NUM_ITER) - float64(v.count)) * 100
            if odds_rest[i] < 0 {
              odds_rest[i] = 0
            }
          }
          //distance := calculateEuclidianDistance(group.odds_to_zone_odds(odds_rest), group.odds_to_zone_odds(v.Pos))
          distance := calculateSimilarity(group.odds_to_zone_odds(odds_rest), group.odds_to_zone_odds(v.Pos))
          if g.Id == 273785 {
            log.Printf("%d %d %v %v %v\n", g.Id, g.Away_id, k, v.count, group.odds_to_zone_odds(v.Pos))
            log.Printf("%d %d %v %v\n", g.Id, g.Away_id, NUM_ITER - v.count, group.odds_to_zone_odds(odds_rest))
            log.Printf("distance %v\n", distance)
          }
          away_importance += (distance) * float64(v.count) / NUM_ITER
        }
      }
      home_importances[g.Id] = home_importance
      away_importances[g.Id] = away_importance
      game_importances[g.Id] = (home_importance + away_importance) / 2
    }
  }
  log.Println("second", time.Since(start2))
  importances := make(map[int][2]float64)
  for k, _ := range game_importances {
    importances[k] = [2]float64{home_importances[k], away_importances[k]}
  }
  result["game_importance"] = importances
  log.Println("time elapsed", elapsed)
  log.Println("time elapsed", time.Since(start_func))
  return result
}

type Pair struct {
  Key int
  Value float64
}

type PairList []Pair

func (p PairList) Len() int { return len(p) }
func (p PairList) Less(i, j int) bool { return p[i].Value < p[j].Value }
func (p PairList) Swap(i, j int){ p[i], p[j] = p[j], p[i] }

func calculateEuclidianDistance(a, b []float64) float64 {
  var distance float64
  for i, _ := range a {
    distance += (a[i]-b[i])*(a[i]-b[i])
  }
  return math.Sqrt(distance)
}

func calculateSimilarity(a, b []float64) float64 {
  var similarity float64
  var divisor1, divisor2 float64
  for i, _ := range a {
    x := math.Pow(a[i] / 100, 0.5)
    y := math.Pow(b[i] / 100, 0.5)
    similarity += x*y
    divisor1 += x*x
    divisor2 += y*y
  }
  similarity /= math.Sqrt(divisor1) * math.Sqrt(divisor2)
  return math.Sqrt(1 - similarity*similarity)
}

func calculateChampionshipOdds(c http.ResponseWriter, req *http.Request) {
  fmt.Printf("New Request\n")
  fmt.Println(req.Body)
  dec := json.NewDecoder(req.Body)
  var v GroupType
  if err := dec.Decode(&v); err != nil {
    fmt.Println(err)
    return
  }
  team_odds := v.calculate_odds()
  enc := json.NewEncoder(c)
  enc.Encode(team_odds)
}

type GamesType struct {
  Games [][]float64
  Ratings map[string][]float64
}

type TeamRating struct {
  Offense float64
  Defense float64
}

func squash_date(timestamp float64, now float64) float64 {
  x := float64(timestamp - now) / (730*24*60*60)
  return 1 + (math.Exp(x)-math.Exp(-x))/(math.Exp(x)+math.Exp(-x))
}

func (all_games *GamesType) spi() map[string]*TeamRating {
  AVG_BASE := 1.3350257653834494
  off_rating := make(map[float64]float64)
  def_rating := make(map[float64]float64)
  NUM_ITER := 100000
  log.Print(len(all_games.Games))
  games := make(map[float64]float64)
  team_weights := make(map[float64]float64)
  team_penalties := make(map[float64]float64)
  weights := make([]float64, len(all_games.Games))
  now := all_games.Games[len(all_games.Games)-1][4]
  for i, g := range all_games.Games {
    weights[i] = g[5] * squash_date(g[4], now)
    games[g[0]] += weights[i]  // Game_weight
    games[g[1]] += weights[i]  // Game_weight
  }
  for k, _ := range games {
    team_weights[k] = team_weight(games[k])
    team_penalties[k] = team_penalty(games[k])
    log.Printf("Team: %0.0f Weight: %0.02f penalty: %0.02f games: %0.02f", k, team_weights[k], team_penalties[k], games[k])
    games[k] = 0
  }
  for i, g := range all_games.Games {
    weights[i] *= team_weights[g[0]] * team_weights[g[1]]
    games[g[0]] += weights[i]  // Game_weight
    games[g[1]] += weights[i]  // Game_weight
  }
  for k, _ := range games {
    games[k] += 1.0
  }
  for k, v := range all_games.Ratings {
    id, _ := strconv.ParseFloat(k, 64)
    off_rating[id] = upper_bound(v[0], games[id])
    def_rating[id] = lower_bound(v[1], games[id])
  }
  good_iterations := 0
  for i := 0; i < NUM_ITER; i++ {
    adjusted_goals_scored := make(map[float64]float64)
    adjusted_goals_allowed := make(map[float64]float64)
    for k, _ := range games {
      adjusted_goals_scored[k] += AVG_BASE
      adjusted_goals_allowed[k] += AVG_BASE
    }
    for i, g := range all_games.Games {
      home_adv := g[6]
      adjusted_goals_scored[g[0]] += team_penalties[g[0]] * weights[i] * ((float64(g[2]) - (def_rating[g[1]]+home_adv))/( math.Max(0.25, (def_rating[g[1]]+home_adv)*0.424+0.548) )*(AVG_BASE*0.424+0.548)+AVG_BASE)
      adjusted_goals_allowed[g[0]] += (1/team_penalties[g[0]]) * weights[i] * ((float64(g[3]) - (off_rating[g[1]]-home_adv))/( math.Max(0.25, (off_rating[g[1]]-home_adv)*0.424+0.548) )*(AVG_BASE*0.424+0.548)+AVG_BASE)
      adjusted_goals_scored[g[1]] += team_penalties[g[1]] * weights[i] * ((float64(g[3]) - (def_rating[g[0]]-home_adv))/( math.Max(0.25, (def_rating[g[0]]-home_adv)*0.424+0.548) )*(AVG_BASE*0.424+0.548)+AVG_BASE)
      adjusted_goals_allowed[g[1]] += (1/team_penalties[g[1]]) * weights[i] * ((float64(g[2]) - (off_rating[g[0]]+home_adv))/( math.Max(0.25, (off_rating[g[0]]+home_adv)*0.424+0.548) )*(AVG_BASE*0.424+0.548)+AVG_BASE)
    }
    error := 0.0
    largest_k := 0.0
    for k, _ := range games {
      old_off := off_rating[k]
      off_rating[k] = adjusted_goals_scored[k] / games[k]
      def_rating[k] = adjusted_goals_allowed[k] / games[k]
      this_error := math.Sqrt((old_off - off_rating[k])*(old_off - off_rating[k]))
      if (this_error > error) {
        error = this_error
        largest_k = k
      }
    }
    if (error < 0.0001) {
      good_iterations += 1
    } else {
      good_iterations = 0
    }
    if (good_iterations > 10) {
      log.Print(i)
      log.Printf("%f: %f", largest_k, error)
      log.Printf("%f %.6f %.6f", largest_k, off_rating[largest_k], def_rating[largest_k])
      break
    }
    if (i % 10 == 0 || i % 10 == 1) {
      log.Print(i)
      log.Printf("%f: %f", largest_k, error)
      log.Printf("%f %.6f %.6f %.6f", largest_k, off_rating[largest_k], def_rating[largest_k], games[largest_k])
    }
  }
  ratings := make(map[string]*TeamRating)
//  for k, _ := range games {
//    off_rating[k] = (off_rating[k] * games[k] + 5*AVG_BASE) / (games[k] + 5)
//    def_rating[k] = (def_rating[k] * games[k] + 5*AVG_BASE) / (games[k] + 5)
//  }
  for k, _ := range off_rating {
    key := strconv.FormatFloat(k, 'f', -1, 64)
    ratings[key] = new(TeamRating)
    ratings[key].Offense = lower_bound(off_rating[k], games[k])
    ratings[key].Defense = upper_bound(def_rating[k], games[k])
  }
  return ratings
}

func lower_bound(avg float64, sample float64) float64 {
  return avg / (1.0 + math.Exp(-sample / 4))
}

func upper_bound(avg float64, sample float64) float64 {
  return avg * (1.0 + math.Exp(-sample / 4))
}

func team_weight(games float64) float64 {
  return  1 / (1.0 + math.Exp(-games / 4)) * 2 - 1
}

func team_penalty(games float64) float64 {
  return  1 / (1.0 + math.Exp(-games / 4)) / 2.5 + 0.6
}

func calculatePowerRanking(c http.ResponseWriter, req *http.Request) {
  fmt.Printf("New Request\n")
  fmt.Println(req.Body)
  dec := json.NewDecoder(req.Body)
  var v GamesType
  if err := dec.Decode(&v); err != nil {
    fmt.Println(err)
    return
  }
  ratings := v.spi()
//  enc2 := json.NewEncoder(os.Stdout)
//  enc2.Encode(ratings)
  enc := json.NewEncoder(c)
  enc.Encode(ratings)
}


// MPH


// Table stores the values for the hash function
type Table struct {
    values []int32
    seeds  []int32
}

type entry struct {
    idx  int32
    hash uint64
}

// NewTable constructs a minimal perfect hash function for the set of keys which returns the index of item in the keys array.
func NewTable(keys []uint32) *Table {
    size := uint64(nextPower2(len(keys)))

    h := make([][]entry, size)
    for idx, k := range keys {
        var bs [4]byte
        binary.LittleEndian.PutUint32(bs[:], k)
        hash := metro.Hash64(bs[:], 0)
        i := hash % size
        // idx+1 so we can identify empty entries in the table with 0
        h[i] = append(h[i], entry{int32(idx) + 1, hash})
    }

    sort.Slice(h, func(i, j int) bool { return len(h[i]) > len(h[j]) })

    values := make([]int32, size)
    seeds := make([]int32, size)

    var hidx int
    for hidx = 0; hidx < len(h) && len(h[hidx]) > 1; hidx++ {
        subkeys := h[hidx]

        var seed uint64
        entries := make(map[uint64]int32)

    newseed:
        for {
            seed++
            for _, k := range subkeys {
                i := xorshiftMult64(k.hash+seed) % size
                if entries[i] == 0 && values[i] == 0 {
                    // looks free, claim it
                    entries[i] = k.idx
                    continue
                }

                // found a collision, reset and try a new seed
                for k := range entries {
                    delete(entries, k)
                }
                continue newseed
            }

            // made it through; everything got placed
            break
        }

        // mark subkey spaces as claimed
        for k, v := range entries {
            values[k] = v
        }

        // and assign this seed value for every subkey
        for _, k := range subkeys {
            i := k.hash % size
            seeds[i] = int32(seed)
        }
    }

    // find the unassigned entries in the table
    var free []int
    for i := range values {
        if values[i] == 0 {
            free = append(free, i)
        } else {
            // decrement idx as this is now the final value for the table
            values[i]--
        }
    }

    for len(h[hidx]) > 0 {
        k := h[hidx][0]
        i := k.hash % size
        hidx++

        // take a free slot
        dst := free[0]
        free = free[1:]

        // claim it; -1 because of the +1 at the start
        values[dst] = k.idx - 1

        // store offset in seed as a negative; -1 so even slot 0 is negative
        seeds[i] = -int32(dst + 1)
    }

    return &Table{
        values: values,
        seeds:  seeds,
    }
}

// Query looks up an entry in the table and return the index.
func (t *Table) Query(k uint32) int32 {
    size := uint64(len(t.values))
    var bs [4]byte
    binary.LittleEndian.PutUint32(bs[:], k)
    hash := metro.Hash64(bs[:], 0)
    i := hash & (size - 1)
    seed := t.seeds[i]
    if seed < 0 {
        return t.values[-seed-1]
    }

    i = xorshiftMult64(uint64(seed)+hash) & (size - 1)
    return t.values[i]
}

func xorshiftMult64(x uint64) uint64 {
    x ^= x >> 12 // a
    x ^= x << 25 // b
    x ^= x >> 27 // c
    return x * 2685821657736338717
}

func nextPower2(n int) int {
    i := 1
    for i < n {
        i *= 2
    }
    return i
}

func main() {
  fmt.Printf("Starting http Server ... ")
  http.Handle("/odds", http.HandlerFunc(calculateChampionshipOdds))
  http.Handle("/spi", http.HandlerFunc(calculatePowerRanking))
  log.Fatal(http.ListenAndServe(":6577", nil))
}
