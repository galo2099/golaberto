package main

import _ "http/pprof"
import "fmt"
import "http"
import "json"
import "math"
import "rand"
import "os"
import "sort"
import "strconv"
import "strings"
import "log"

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
  home_games []GameType
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
  Id int
}

type GroupType struct {
  Promoted int
  Relegated int
  Phase *PhaseType
  Games []GameType
  Teams []TeamType
}


func factorial(x int) int {
  ret := 1
  for i := 2; i <= x; i++  {
    ret *= i
  }
  return ret
}

func find_poisson_mean(distribution []float64) float64 {
  nn := 0.
  for i := range distribution {
    nn += distribution[i]
  }

  cs := 10000000000000000.
  mean := 0.
  rawp := make([]float64, len(distribution))
  exp := make([]float64, len(distribution))

  for i := 0; i < 15000; i++ {
    csnew := 0.
    mean += 0.01

    for j := range distribution {
      rawp[j] = math.Exp(-mean) * math.Pow(mean, float64(j)) / float64(factorial(j))
      exp[j] = rawp[j] * nn;
      csnew += math.Exp2(distribution[j] - exp[j])
    }

    if csnew < cs {
      cs = csnew
    } else {
      mean -= 0.01
      break
    }
  }
  return mean
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

type GameOdds struct {
  Home_id int
  Away_id int
  Home_mean float64
  Away_mean float64
}

func (group *GroupType) goal_distribution(
    team_id int,
    side func (*GameType) int,
    score func (*GameType) int) []float64 {
  ret := make([]float64, 10)
  for _, g := range group.Games {
    if g.Played && side(&g) == team_id {
      for i := range ret {
        ret[i] = ret[i] * 0.8;
      }
      goals := score(&g)
      if goals > 9 {
        goals = 9
      }
      ret[goals] += 1
    }
  }
  return ret
}

func (group *GroupType) home_for(team_id int) []float64 {
  return group.goal_distribution(team_id,
                           func (g *GameType) int { return g.Home_id },
                           func (g *GameType) int { return g.Home_score})
}

func (group *GroupType) home_against(team_id int) []float64 {
  return group.goal_distribution(team_id,
                           func (g *GameType) int { return g.Home_id },
                           func (g *GameType) int { return g.Away_score})
}

func (group *GroupType) away_for(team_id int) []float64 {
  return group.goal_distribution(team_id,
                           func (g *GameType) int { return g.Away_id },
                           func (g *GameType) int { return g.Away_score})
}

func (group *GroupType) away_against(team_id int) []float64 {
  return group.goal_distribution(team_id,
                           func (g *GameType) int { return g.Away_id },
                           func (g *GameType) int { return g.Home_score})
}

func add_slices(x, y []float64) []float64 {
  for i := range x {
    x[i] += y[i]
  }
  return x
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
      case "head":
        a, b = 0, 0
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

type TeamOdds struct {
  First float64
  Promoted float64
  Relegated float64
}

func (group *GroupType) calculate_odds() map[string]*TeamOdds {
  odds := make([]GameOdds, 0, len(group.Games))
  campaign := make(map[int]*TeamCampaign, len(group.Teams))

  for _, t := range group.Teams {
    campaign[t.Id] = &TeamCampaign{id: t.Id,
      points_win: group.Phase.Championship.Point_win,
      points_draw: group.Phase.Championship.Point_draw,
      points_loss: group.Phase.Championship.Point_loss}
  }

  for _, g := range group.Games {
    if g.Played {
      if campaign[g.Home_id] != nil {
        campaign[g.Home_id].add_game(&g)
      }
      if campaign[g.Away_id] != nil {
        campaign[g.Away_id].add_game(&g)
      }
    } else {
      odds = append(odds, GameOdds{g.Home_id, g.Away_id,
        find_poisson_mean(add_slices(group.home_for(g.Home_id), group.away_against(g.Away_id))),
        find_poisson_mean(add_slices(group.away_for(g.Away_id), group.home_against(g.Home_id))) })
    }
  }

  sort_order := strings.Split(group.Phase.Sort, ",")
  team_odds := make(map[int]*TeamOdds, len(campaign))
  for k, _ := range campaign {
    team_odds[k] = new(TeamOdds)
  }
  const NUM_ITER = 10000
  for i := 0; i < NUM_ITER; i++ {
    simulated_campaign := make(map[int]*TeamCampaign, len(campaign))
    for k, v := range campaign {
      simulated_campaign[k] = v.clone()
    }

    for _, v := range odds {
      home_score := poisson_rand(v.Home_mean)
      away_score := poisson_rand(v.Away_mean)
      if simulated_campaign[v.Home_id] != nil {
        simulated_campaign[v.Home_id].add_game(&GameType{v.Home_id, v.Away_id, home_score, away_score, true})
      }
      if simulated_campaign[v.Away_id] != nil {
        simulated_campaign[v.Away_id].add_game(&GameType{v.Home_id, v.Away_id, home_score, away_score, true})
      }
    }

    team_slice := make([]*TeamCampaign, 0, len(simulated_campaign));
    for _, v := range simulated_campaign {
      team_slice = append(team_slice, v)
    }

    sorted_teams := TeamCampaignSorted{team_slice, sort_order}
    sort.Sort(sorted_teams)
    for i, v := range sorted_teams.t {
      if i == 0 {
        team_odds[v.id].First += 1
      }
      if i < group.Promoted {
        team_odds[v.id].Promoted += 1
      }
      if i >= len(sorted_teams.t) - group.Relegated {
        team_odds[v.id].Relegated += 1
      }
    }
  }
  json_team_odds := make(map[string]*TeamOdds, len(team_odds))
  for k, v := range team_odds {
    v.First /= NUM_ITER / 100
    v.Promoted /= NUM_ITER / 100
    v.Relegated /= NUM_ITER / 100
    json_team_odds[strconv.Itoa(k)] = v
  }
  return json_team_odds
}

func serveRequests(c http.ResponseWriter, req *http.Request) {
  fmt.Printf("New Request\n")
  fmt.Println(req.Body)
  dec := json.NewDecoder(req.Body)
  var v GroupType
  if err := dec.Decode(&v); err != nil {
    log.Println(err)
    return
  }
  team_odds := v.calculate_odds()
  enc := json.NewEncoder(c)
  enc.Encode(team_odds)
  enc2 := json.NewEncoder(os.Stdout)
  enc2.Encode(team_odds)
}

func main() {
  fmt.Printf("Starting http Server ... ")
  http.Handle("/", http.HandlerFunc(serveRequests))
  err := http.ListenAndServe("127.0.0.1:6577", nil)
  if err != nil {
    fmt.Printf("ListenAndServe Error :" + err.String())
  }
}
