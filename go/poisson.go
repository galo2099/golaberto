package main

import _ "net/http/pprof"
import "fmt"
import "net/http"
import "encoding/json"
import "log"
import "math"
import "math/rand"
import "os"
import "sort"
import "strconv"
import "strings"

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
  t.home_games = make([]*GameType, len(campaign.home_games))
  copy(t.home_games, campaign.home_games)
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
    campaign.home_games = append(campaign.home_games, game)
    campaign.goals_for += game.Home_score
    campaign.goals_against += game.Away_score
  } else {
    campaign.goals_for += game.Away_score
    campaign.goals_against += game.Home_score
    campaign.goals_away += game.Away_score
  }
}

type GameType struct {
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
  Games []GameType
}

type TeamType struct {
  Team_id int
  Add_sub int
  Bias int
}

type GroupType struct {
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

type TeamCampaignSorted struct {
  t []*TeamCampaign
  sort []string
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
      case "pt":
        a, b = float64(sorted.t[i].points), float64(sorted.t[j].points)
      case "w":
        a, b = float64(sorted.t[i].wins), float64(sorted.t[j].wins)
      case "gd":
        a, b = float64(sorted.t[i].goals_diff()), float64(sorted.t[j].goals_diff())
      case "gf":
        a, b = float64(sorted.t[i].goals_for), float64(sorted.t[j].goals_for)
      case "g_average":
        a, b = float64(sorted.t[i].goals_average()), float64(sorted.t[j].goals_average())
      case "g_away":
        a, b = float64(sorted.t[i].goals_away), float64(sorted.t[j].goals_away)
      case "bias":
        a, b = float64(sorted.t[i].bias), float64(sorted.t[j].bias)
      case "g_aet":
        a, b = 0, 0
      case "gp":
        a, b = 0, 0
      case "head":
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

func compare_head_to_head(a, b *TeamCampaign, sort []string) int {
  head_to_head_campaign := make([]*TeamCampaign, 2)
  head_to_head_campaign[0] = &TeamCampaign{id: a.id,
      points_win: a.points_win,
      points_draw: a.points_draw,
      points_loss: a.points_loss}
  head_to_head_campaign[1] = &TeamCampaign{id: b.id,
      points_win: a.points_win,
      points_draw: a.points_draw,
      points_loss: a.points_loss}
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
  sort_no_head := make([]string, 0, len(sort))
  for _, s := range sort {
    if s != "name" && s != "head" {
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
  Pos []float64
}

func (group *GroupType) calculate_odds() map[string]*TeamOdds {
  campaign := make(map[int]*TeamCampaign, len(group.Team_groups))

  for _, t := range group.Team_groups {
    campaign[t.Team_id] = &TeamCampaign{id: t.Team_id,
      points: t.Add_sub,
      bias: t.Bias,
      add_sub: t.Add_sub,
      points_win: group.Phase.Championship.Point_win,
      points_draw: group.Phase.Championship.Point_draw,
      points_loss: group.Phase.Championship.Point_loss}
  }

  for i := range group.Games {
    if group.Games[i].Played {
      if campaign[group.Games[i].Home_id] != nil {
        campaign[group.Games[i].Home_id].add_game(&group.Games[i])
      }
      if campaign[group.Games[i].Away_id] != nil {
        campaign[group.Games[i].Away_id].add_game(&group.Games[i])
      }
    }
  }

  sort_order := strings.Split(strings.Join(strings.Fields(group.Phase.Sort), ""), ",")
  team_odds := make(map[int]*TeamOdds, len(campaign))
  for k, _ := range campaign {
    team_odds[k] = new(TeamOdds)
    team_odds[k].Pos = make([]float64, len(campaign))
  }
  const NUM_ITER = 10000
  for i := 0; i < NUM_ITER; i++ {
    simulated_campaign := make(map[int]*TeamCampaign, len(campaign))
    for k, v := range campaign {
      simulated_campaign[k] = v.clone()
    }

    for i := range group.Games {
      if !group.Games[i].Played {
        home_score := poisson_rand(group.Games[i].Home_power)
        away_score := poisson_rand(group.Games[i].Away_power)
        if simulated_campaign[group.Games[i].Home_id] != nil {
          simulated_campaign[group.Games[i].Home_id].add_game(&GameType{group.Games[i].Home_id, group.Games[i].Away_id, home_score, away_score, 0.0, 0.0, true})
        }
        if simulated_campaign[group.Games[i].Away_id] != nil {
          simulated_campaign[group.Games[i].Away_id].add_game(&GameType{group.Games[i].Home_id, group.Games[i].Away_id, home_score, away_score, 0.0, 0.0, true})
        }
      }
    }

    team_slice := make([]*TeamCampaign, 0, len(simulated_campaign));
    for _, v := range simulated_campaign {
      team_slice = append(team_slice, v)
    }

    sorted_teams := TeamCampaignSorted{team_slice, sort_order}
    sort.Sort(sorted_teams)
    for i, v := range sorted_teams.t {
      team_odds[v.id].Pos[i] += 1.0
    }
  }
  json_team_odds := make(map[string]*TeamOdds, len(team_odds))
  for team, odds := range team_odds {
    for pos := range odds.Pos {
      odds.Pos[pos] /= NUM_ITER / 100
    }
    json_team_odds[strconv.Itoa(team)] = odds
  }
  return json_team_odds
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
  enc2 := json.NewEncoder(os.Stdout)
  enc2.Encode(team_odds)
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

func main() {
  fmt.Printf("Starting http Server ... ")
  http.Handle("/odds", http.HandlerFunc(calculateChampionshipOdds))
  http.Handle("/spi", http.HandlerFunc(calculatePowerRanking))
  log.Fatal(http.ListenAndServe(":6577", nil))
}
