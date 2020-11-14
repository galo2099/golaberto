package main

import (
	"database/sql"
	"encoding/binary"
	"encoding/json"
	"fmt"
	"log"
	"math"
	"math/rand"
	"net/http"
	_ "net/http/pprof"
	"sort"
	"strconv"
	"strings"
	"time"

	"github.com/dgryski/go-metro"
	_ "github.com/go-sql-driver/mysql"
)

const (
	AVG_BASE = 1.3350257653834494
	HOME_ADV = 0.16133676871779334
)

type TeamCampaign struct {
	id            int
	points        int
	wins          int
	draws         int
	losses        int
	goals_for     int
	goals_against int
	goals_away    int
	bias          int
	add_sub       int
	points_win    int
	points_draw   int
	points_loss   int
	home_games    []*GameType
	uses_head     bool
}

func (campaign *TeamCampaign) goals_diff() int {
	return campaign.goals_for - campaign.goals_against
}

func (campaign *TeamCampaign) goals_average() float64 {
	return float64(campaign.goals_for) / (float64(campaign.goals_against) + 0.0000000000001)
}

func (campaign *TeamCampaign) clone() *TeamCampaign {
	if campaign == nil {
		return nil
	}
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
	is_home := campaign.id == game.HomeId
	if game.HomeScore > game.AwayScore {
		if is_home {
			campaign.wins += 1
			campaign.points += campaign.points_win
		} else {
			campaign.losses += 1
			campaign.points += campaign.points_loss
		}
	} else if game.HomeScore < game.AwayScore {
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
		campaign.goals_for += game.HomeScore
		campaign.goals_against += game.AwayScore
	} else {
		campaign.goals_for += game.AwayScore
		campaign.goals_against += game.HomeScore
		campaign.goals_away += game.AwayScore
	}
}

type GameType struct {
	Id               int     `json:"id"`
	HomeId           int     `json:"home_id"`
	AwayId           int     `json:"away_id"`
	HomeScore        int     `json:"home_score"`
	AwayScore        int     `json:"away_score"`
	HomePower        float64 `json:"home_power"`
	AwayPower        float64 `json:"away_power"`
	Played           bool    `json:"played"`
	home_table_index int32
	away_table_index int32
}

type PhaseType struct {
	Championship           *ChampionshipType
	Bonus_points           int
	Bonus_points_threshold int
	Sort                   string
}

type ChampionshipType struct {
	Point_draw int
	Point_loss int
	Point_win  int
}

type TeamType struct {
	Team_id int
	Add_sub int
	Bias    int
}

type ZoneType struct {
	Position []int
}

type GroupType struct {
	Id          int
	Zones       []ZoneType
	Phase       *PhaseType
	Games       []*GameType
	Team_groups []TeamType
}

func factorial(x float64) float64 {
	ret := 1.0
	for i := 1.0; i <= x; i++ {
		ret *= i
	}
	return ret
}

func poisson_pmf(mean, x float64) float64 {
	return math.Pow(mean, x) * math.Exp(-mean) / factorial(x)
}

func poisson_rand(mean float64) int {
	var em int
	t := 0.0
	for {
		t += rand.ExpFloat64()
		if t >= mean {
			break
		}
		em++
	}
	return em
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
	t    []*TeamCampaign
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
		points_win:  a.points_win,
		points_draw: a.points_draw,
		points_loss: a.points_loss,
		uses_head:   false}
	head_to_head_campaign[1] = &TeamCampaign{id: b.id,
		points_win:  a.points_win,
		points_draw: a.points_draw,
		points_loss: a.points_loss,
		uses_head:   false}
	for i := range a.home_games {
		if a.home_games[i].AwayId == b.id {
			head_to_head_campaign[0].add_game(a.home_games[i])
			head_to_head_campaign[1].add_game(a.home_games[i])
		}
	}
	for i := range b.home_games {
		if b.home_games[i].AwayId == a.id {
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
	count float64
	Pos   []float64
}

type SimulatedScore struct {
	home int
	away int
}

type GameOdds struct {
	game_index int
	scores     []*TeamOdds
}

func entropy(v []float64) float64 {
	ret := 0.0
	for _, x := range v {
		if x > 0 {
			ret += -x * math.Log2(x)
		}
	}
	return ret
}

func jensen(scores []*TeamOdds) float64 {
	var ret float64
	var average_probs []float64
	for _, s := range scores {
		if average_probs == nil {
			average_probs = make([]float64, len(s.Pos))
		}
		ret -= s.count * entropy(s.Pos)
		for i := range s.Pos {
			average_probs[i] += s.count * s.Pos[i]
		}
	}
	ret += entropy(average_probs)
	if ret <= 0.0 {
		return 0.0
	}

	non_zero := 0.0
	for _, v := range average_probs {
		if v > 0 {
			non_zero += 1
		}
	}
	if non_zero == 1 {
		return 0.0
	}
	return ret / math.Max(entropy(average_probs), 1.0)
}

func (group *GroupType) odds_to_zone_odds(odds []float64) []float64 {
	zone_odds := make([]float64, len(group.Zones)+1)
	for i, v := range odds {
		var found bool
		for j, zone := range group.Zones {
			for _, p := range zone.Position {
				if i+1 == p {
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
	return score.home*10 + score.away
}

type OddsType struct {
	team  *TeamOdds
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
	all_team_ids_map := make(map[int]struct{})
	// Some teams may appear in games but not the group
	for _, g := range group.Games {
		all_team_ids_map[g.HomeId] = struct{}{}
		all_team_ids_map[g.AwayId] = struct{}{}
	}
	// Some teams may appear in the group but not games
	for _, t := range group.Team_groups {
		all_team_ids_map[t.Team_id] = struct{}{}
	}
	// hack to avoid crash when only two keys
	if len(all_team_ids_map) == 2 || len(all_team_ids_map) == 4 {
		all_team_ids_map[-1] = struct{}{}
	}
	all_team_ids := make([]uint32, 0, len(group.Team_groups))
	for k, _ := range all_team_ids_map {
		all_team_ids = append(all_team_ids, uint32(k))
	}
	table := NewTable(all_team_ids)
	for _, g := range group.Games {
		g.home_table_index = table.Query(uint32(g.HomeId))
		g.away_table_index = table.Query(uint32(g.AwayId))
	}

	campaign := make([]*TeamCampaign, len(all_team_ids))
	for _, t := range group.Team_groups {
		campaign[table.Query(uint32(t.Team_id))] = &TeamCampaign{id: t.Team_id,
			points:      t.Add_sub,
			bias:        t.Bias,
			add_sub:     t.Add_sub,
			points_win:  group.Phase.Championship.Point_win,
			points_draw: group.Phase.Championship.Point_draw,
			points_loss: group.Phase.Championship.Point_loss,
			uses_head:   uses_head}
	}

	for _, g := range group.Games {
		if g.Played {
			if campaign[g.home_table_index] != nil {
				campaign[g.home_table_index].add_game(g)
			}
			if campaign[g.away_table_index] != nil {
				campaign[g.away_table_index].add_game(g)
			}
		}
	}

	team_odds := make([]OddsType, len(all_team_ids))
	for k := range team_odds {
		team_odds[k] = OddsType{&TeamOdds{0, make([]float64, len(group.Team_groups))}, nil}
	}

	for i, g := range group.Games {
		if !g.Played {
			team_odds[g.home_table_index].games = append(team_odds[g.home_table_index].games, &GameOdds{i, make([]*TeamOdds, 100)})
			team_odds[g.away_table_index].games = append(team_odds[g.away_table_index].games, &GameOdds{i, make([]*TeamOdds, 100)})
		}
	}

	var elapsed time.Duration
	var elapsed2 time.Duration
	simulated_scores := make([]SimulatedScore, len(group.Games))
	simulated_campaign := make([]*TeamCampaign, len(all_team_ids))
	team_slice := make([]*TeamCampaign, len(group.Team_groups))
	const NUM_ITER = 10000
	for i := 0; i < NUM_ITER; i++ {
		for k, v := range campaign {
			simulated_campaign[k] = v.clone()
		}

		start3 := time.Now()
		for i, g := range group.Games {
			if !g.Played {
				home_score := poisson_rand(g.HomePower)
				away_score := poisson_rand(g.AwayPower)
				simulated_scores[i] = SimulatedScore{home_score, away_score}
				home := g.home_table_index
				away := g.away_table_index
				if simulated_campaign[home] != nil {
					simulated_campaign[home].add_game(&GameType{g.Id, g.HomeId, g.AwayId, home_score, away_score, 0.0, 0.0, true, home, away})
				}
				if simulated_campaign[away] != nil {
					simulated_campaign[away].add_game(&GameType{g.Id, g.HomeId, g.AwayId, home_score, away_score, 0.0, 0.0, true, home, away})
				}
			}
		}
		elapsed2 += time.Since(start3)

		{
			i := 0
			for _, v := range simulated_campaign {
				if v != nil {
					team_slice[i] = v
					i += 1
				}
			}
		}

		sorted_teams := TeamCampaignSorted{team_slice, sort_order}
		sort.Sort(sorted_teams)

		start := time.Now()
		for i, v := range sorted_teams.t {
			team := team_odds[table.Query(uint32(v.id))]
			team.team.Pos[i] += 1.0 / NUM_ITER
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
	} // end for

	home_importances := make(map[int]float64)
	away_importances := make(map[int]float64)
	start2 := time.Now()
	for i, g := range group.Games {
		if !g.Played {
			home := g.home_table_index
			away := g.away_table_index
			var home_importance float64
			//      home_odds := group.odds_to_zone_odds(team_odds[home].team.Pos)
			for _, x := range team_odds[home].games {
				if x.game_index != i {
					continue
				}
				for _, v := range x.scores {
					if v == nil {
						continue
					}
					for j, _ := range v.Pos {
						v.Pos[j] /= float64(v.count)
					}
				}
				zone_odds := make([]*TeamOdds, 0, len(x.scores))
				for _, v := range x.scores {
					if v == nil {
						continue
					}
					zone_odds = append(zone_odds, &TeamOdds{count: v.count / NUM_ITER, Pos: group.odds_to_zone_odds(v.Pos)})
				}
				//        for _, v := range zone_odds {
				//          home_importance += v.count * calculateEuclidianDistance(v.Pos, home_odds) * math.Sqrt(2)
				//        }
				home_importance = math.Sqrt(jensen(zone_odds))
			}
			var away_importance float64
			//      away_odds := group.odds_to_zone_odds(team_odds[away].team.Pos)
			for _, x := range team_odds[away].games {
				if x.game_index != i {
					continue
				}
				for _, v := range x.scores {
					if v == nil {
						continue
					}
					for j, _ := range v.Pos {
						v.Pos[j] /= float64(v.count)
					}
				}
				zone_odds := make([]*TeamOdds, 0, len(x.scores))
				for _, v := range x.scores {
					if v == nil {
						continue
					}
					zone_odds = append(zone_odds, &TeamOdds{count: v.count / NUM_ITER, Pos: group.odds_to_zone_odds(v.Pos)})
				}
				//        for _, v := range zone_odds {
				//          away_importance += v.count * calculateEuclidianDistance(v.Pos, away_odds) * math.Sqrt(2)
				//        }
				away_importance = math.Sqrt(jensen(zone_odds))
			}
			home_importances[g.Id] = home_importance
			away_importances[g.Id] = away_importance
		}
	}
	log.Println("second", time.Since(start2))
	importances := make(map[int]interface{})
	teams_in_group := make(map[int]struct{})
	// We'll only update the importance for teams that are in the group
	for _, t := range group.Team_groups {
		teams_in_group[t.Team_id] = struct{}{}
	}
	for _, g := range group.Games {
		if !g.Played {
			_, home := teams_in_group[g.HomeId]
			_, away := teams_in_group[g.AwayId]
			if home && away {
				importances[g.Id] = [2]float64{home_importances[g.Id], away_importances[g.Id]}
			} else if home {
				importances[g.Id] = [2]interface{}{home_importances[g.Id], nil}
			} else if away {
				importances[g.Id] = [2]interface{}{nil, away_importances[g.Id]}
			}
		}
	}
	json_team_odds := make(map[int]*TeamOdds, len(team_odds))
	for _, team := range campaign {
		if team == nil {
			continue
		}
		index := table.Query(uint32(team.id))
		json_team_odds[team.id] = team_odds[index].team
		for i := range json_team_odds[team.id].Pos {
			json_team_odds[team.id].Pos[i] = json_team_odds[team.id].Pos[i] * 100
		}
	}
	result := make(map[string]interface{})
	result["team_odds"] = json_team_odds
	result["game_importance"] = importances
	log.Println("time elapsed", elapsed)
	log.Println("time elapsed", elapsed2)
	log.Println("time elapsed", time.Since(start_func))
	log.Println("Group", group.Id)
	return result
}

type Pair struct {
	Key   int
	Value float64
}

type PairList []Pair

func (p PairList) Len() int           { return len(p) }
func (p PairList) Less(i, j int) bool { return p[i].Value < p[j].Value }
func (p PairList) Swap(i, j int)      { p[i], p[j] = p[j], p[i] }

func calculateEuclidianDistance(a, b []float64) float64 {
	var distance float64
	for i, _ := range a {
		distance += (a[i] - b[i]) * (a[i] - b[i])
	}
	return math.Sqrt(distance)
}

func calculateSimilarity(a, b []float64) float64 {
	var similarity float64
	var divisor1, divisor2 float64
	for i, _ := range a {
		x := a[i]
		y := b[i]
		similarity += x * y
		divisor1 += x * x
		divisor2 += y * y
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

type SpiGameType struct {
	Phase_Id         int
	Home_Id          int
	Away_Id          int
	Home_Score       float64
	Away_Score       float64
	Timestamp        int64
	Length           float64
	Advantage        float64
	home_table_index int32
	away_table_index int32
}

type GamesType struct {
	Phases_To_Eval []int
	Games          []SpiGameType
	Ratings        []*TeamRating
}

type TeamRating struct {
	Id      int
	Offense float64
	Defense float64
	Team    float64
}

func squash_date(timestamp float64, now float64) float64 {
	x := float64(timestamp-now) / (730 * 24 * 60 * 60)
	return 1 + (math.Exp(x)-math.Exp(-x))/(math.Exp(x)+math.Exp(-x))
}

func calculate_power(off_rating, def_rating, advantage float64) float64 {
	return math.Min(10.0, math.Max(0.01, (off_rating-AVG_BASE)/(AVG_BASE*0.424+0.548)*((def_rating+advantage)*0.424+0.548)+(def_rating+advantage)))
}

func (g *SpiGameType) home_power(r map[int]*TeamRating) float64 {
	off_rating := AVG_BASE / 2
	if r[int(g.Home_Id)] != nil {
		off_rating = r[int(g.Home_Id)].Offense
	}
	def_rating := AVG_BASE * 2
	if r[int(g.Away_Id)] != nil {
		def_rating = r[int(g.Away_Id)].Defense
	}
	return calculate_power(off_rating, def_rating, g.Advantage)
}

func (g *SpiGameType) away_power(r map[int]*TeamRating) float64 {
	off_rating := AVG_BASE / 2
	if r[int(g.Away_Id)] != nil {
		off_rating = r[int(g.Away_Id)].Offense
	}
	def_rating := AVG_BASE * 2
	if r[int(g.Home_Id)] != nil {
		def_rating = r[int(g.Home_Id)].Defense
	}
	return calculate_power(off_rating, def_rating, g.Advantage)
}

func calculate_game_odds(home_power, away_power float64) (float64, float64, float64) {
	var home, draw, away float64
	for i := 0; i < 20; i++ {
		for j := 0; j < 20; j++ {
			p := poisson_pmf(home_power, float64(i)) * poisson_pmf(away_power, float64(j))
			if i > j {
				home += p
			} else if i == j {
				draw += p
			} else {
				away += p
			}
		}
	}
	return home, draw, away
}

func (g *SpiGameType) odds(r map[int]*TeamRating) (float64, float64, float64) {
	home_power := g.home_power(r)
	away_power := g.away_power(r)
	return calculate_game_odds(home_power, away_power)
}

func (g *SpiGameType) observed_pmf() (float64, float64, float64) {
	if g.Home_Score > g.Away_Score {
		return 1.0, 0.0, 0.0
	} else if g.Home_Score == g.Away_Score {
		return 0.0, 1.0, 0.0
	} else {
		return 0.0, 0.0, 1.0
	}
}

func (g *SpiGameType) rps(r map[int]*TeamRating) float64 {
	home, draw, _ := g.odds(r)
	observed_home, observed_draw, _ := g.observed_pmf()

	return (math.Pow(home-observed_home, 2.0) + math.Pow(home+draw-observed_home-observed_draw, 2.0)) / 2.0
}

func checkItemInSlice(a int, list []int) bool {
	for _, b := range list {
		if b == a {
			return true
		}
	}
	return false
}

func team_rating(r *TeamRating) float64 {
	if r == nil {
		return 0.0
	}
	home_power := calculate_power(r.Offense, AVG_BASE, 0.0)
	away_power := calculate_power(AVG_BASE, r.Defense, 0.0)
	h, d, _ := calculate_game_odds(home_power, away_power)
	return (h*3 + d) / 3 * 100
}

func (all_games *GamesType) historicRatings() map[string]interface{} {
	dates := make([]time.Time, 0)
	ratings_per_team := make(map[int][]float64)
	offense_per_team := make(map[int][]float64)
	defense_per_team := make(map[int][]float64)
	ratings := make(map[int]*TeamRating, len(all_games.Ratings))
	for _, v := range all_games.Ratings {
		ratings[v.Id] = v
	}

	all_team_ids := make([]uint32, 0, len(ratings))
	for k, _ := range ratings {
		all_team_ids = append(all_team_ids, uint32(k))
	}
	table := NewTable(all_team_ids)

	weeks := int64(4 * 52 * 7 * 24 * 60 * 60)
	start_date := all_games.Games[0].Timestamp
	var gamesDeque []*SpiGameType
	for i, _ := range all_games.Games {
		g := &all_games.Games[i]
		g.home_table_index = table.Query(uint32(g.Home_Id))
		g.away_table_index = table.Query(uint32(g.Away_Id))
		gamesDeque = append(gamesDeque, g)
		for gamesDeque[0].Timestamp < g.Timestamp-4*365*24*3600 {
			gamesDeque = gamesDeque[1:]
		}
		if g.Timestamp-start_date > weeks || i == len(all_games.Games)-1 {
			weeks += 1 * 24 * 60 * 60
			dateUTC := time.Unix(g.Timestamp, 0).UTC()
			dates = append(dates, dateUTC)
			log.Println(dateUTC)
			ratings = spi(gamesDeque, ratings, table)
			for k, v := range ratings {
				ratings_per_team[k] = append(ratings_per_team[k], team_rating(v))
				if v == nil {
					offense_per_team[k] = append(offense_per_team[k], 0.0)
					defense_per_team[k] = append(defense_per_team[k], 0.0)
				} else {
					offense_per_team[k] = append(offense_per_team[k], v.Offense)
					defense_per_team[k] = append(defense_per_team[k], v.Defense)
				}
			}
		}
	}

	startFunc := time.Now()
	count := 0
	var insertSql strings.Builder
	insertSql.WriteString("INSERT INTO historical_ratings (team_id,off_rating,def_rating,rating,measure_date) VALUES ")
	first := true
	for k, v := range ratings_per_team {
		for i, r := range v {
			if first {
				first = false
			} else {
				insertSql.WriteString(",")
			}
			insertSql.WriteString(fmt.Sprintf("(%d, %f, %f, %f, '%s')", k, offense_per_team[k][i], defense_per_team[k][i], r, dates[i].Format("2006-01-02")))
		}
		count += 1
		if count % 10 == 0 {
			insertSql.WriteString(" ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating);")
			_, err := db.Exec(insertSql.String())
			if err != nil {
				panic(err.Error()) // proper error handling instead of panic in your app
			}
			insertSql.Reset()
			insertSql.WriteString("INSERT INTO historical_ratings (team_id,off_rating,def_rating,rating,measure_date) VALUES ")
			first = true
			log.Println(time.Since(startFunc))
		}
	}
	insertSql.WriteString(" ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating);")
	log.Println(time.Since(startFunc))
	_, err := db.Exec(insertSql.String())
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	log.Println(time.Since(startFunc))

	return map[string]interface{}{
		"ratings": map[string]interface{}{},
		"offense": map[string]interface{}{},
		"defense": map[string]interface{}{},
		"dates":   []interface{}{},
	}
}

func (all_games *GamesType) eval() map[string]interface{} {
	rps_per_team := make(map[int]float64)
	games_per_team := make(map[int]int)
	ratings := make(map[int]*TeamRating, len(all_games.Ratings))
	for _, v := range all_games.Ratings {
		ratings[v.Id] = v
	}

	all_team_ids := make([]uint32, 0, len(ratings))
	for k, _ := range ratings {
		all_team_ids = append(all_team_ids, uint32(k))
	}
	table := NewTable(all_team_ids)

	ranked_score := 0.0
	games := 0
	var gamesDeque []*SpiGameType
	var last_spi time.Time
	for i, _ := range all_games.Games {
		g := &all_games.Games[i]
		g.home_table_index = table.Query(uint32(g.Home_Id))
		g.away_table_index = table.Query(uint32(g.Away_Id))
		if checkItemInSlice(g.Phase_Id, all_games.Phases_To_Eval) {
			if time.Unix(int64(g.Timestamp), 0).Day() != last_spi.Day() {
				log.Println(time.Unix(int64(g.Timestamp), 0))
				log.Println("rps", ranked_score/float64(games), games)
				ratings = spi(gamesDeque, ratings, table)
				last_spi = time.Unix(int64(g.Timestamp), 0)
			}
			rps := g.rps(ratings)
			rps_per_team[int(g.Home_Id)] += rps
			games_per_team[int(g.Home_Id)] += 1
			rps_per_team[int(g.Away_Id)] += rps
			games_per_team[int(g.Away_Id)] += 1
			ranked_score += rps
			games++
		}
		gamesDeque = append(gamesDeque, g)
		for gamesDeque[0].Timestamp < g.Timestamp-4*365*24*3600 {
			gamesDeque = gamesDeque[1:]
		}
	}
	for k, _ := range rps_per_team {
		rps_per_team[k] /= float64(games_per_team[k])
	}
	return map[string]interface{}{
		"rps":      ranked_score / float64(games),
		"team_rps": rps_per_team,
	}
}

func spi(games []*SpiGameType, ratings map[int]*TeamRating, table *Table) map[int]*TeamRating {
	NUM_ITER := 100000
	off_rating := make([]float64, len(ratings))
	def_rating := make([]float64, len(ratings))
	log.Println(len(games))
	gamesCount := make([]float64, len(ratings))
	team_weights := make([]float64, len(ratings))
	team_penalties := make([]float64, len(ratings))
	weights := make([]float64, len(games))
	now := games[len(games)-1].Timestamp
	for i := 0; i < len(games); i++ {
		g := games[i]
		weights[i] = g.Length * squash_date(float64(g.Timestamp), float64(now))
		gamesCount[g.home_table_index] += weights[i] // Game_weight
		gamesCount[g.away_table_index] += weights[i] // Game_weight
	}
	for k, _ := range gamesCount {
		team_weights[k] = team_weight(gamesCount[k])
		team_penalties[k] = team_penalty(gamesCount[k])
		//    log.Printf("Team: %0.0f Weight: %0.02f penalty: %0.02f games: %0.02f", k, team_weights[k], team_penalties[k], games[k])
		gamesCount[k] = 0
	}
	for i := 0; i < len(games); i++ {
		g := games[i]
		weights[i] *= team_weights[g.home_table_index] * team_weights[g.away_table_index]
		gamesCount[g.home_table_index] += weights[i] // Game_weight
		gamesCount[g.away_table_index] += weights[i] // Game_weight
	}
	for k, _ := range gamesCount {
		gamesCount[k] += math.Min(1.0, gamesCount[k])
	}
	for k, v := range ratings {
		if v != nil {
			k := table.Query(uint32(k))
			off_rating[k] = upper_bound(v.Offense, gamesCount[k])
			def_rating[k] = lower_bound(v.Defense, gamesCount[k])
		}
	}
	good_iterations := 0
	adjusted_goals_scored := make([]float64, len(ratings))
	adjusted_goals_allowed := make([]float64, len(ratings))
	timer := time.Now()
	for i := 0; i < NUM_ITER; i++ {
		for k, _ := range gamesCount {
			adjusted_goals_scored[k] = AVG_BASE * math.Min(1.0, gamesCount[k]/2)
			adjusted_goals_allowed[k] = AVG_BASE * math.Min(1.0, gamesCount[k]/2)
		}
		for i, g := range games {
			home_adv := g.Advantage
			h_off := off_rating[g.home_table_index]
			h_def := def_rating[g.home_table_index]
			a_off := off_rating[g.away_table_index]
			a_def := def_rating[g.away_table_index]
			w := weights[i]
			h_penalty := team_penalties[g.home_table_index]
			a_penalty := team_penalties[g.away_table_index]
			adjusted_goals_scored[g.home_table_index] += h_penalty * w * ((g.Home_Score-(a_def+home_adv))/((a_def+home_adv)*0.424+0.548)*(AVG_BASE*0.424+0.548) + AVG_BASE)
			adjusted_goals_allowed[g.home_table_index] += (1 / h_penalty) * w * ((g.Away_Score-(a_off-home_adv))/((a_off-home_adv)*0.424+0.548)*(AVG_BASE*0.424+0.548) + AVG_BASE)
			adjusted_goals_scored[g.away_table_index] += a_penalty * w * ((g.Away_Score-(h_def-home_adv))/((h_def-home_adv)*0.424+0.548)*(AVG_BASE*0.424+0.548) + AVG_BASE)
			adjusted_goals_allowed[g.away_table_index] += (1 / a_penalty) * w * ((g.Home_Score-(h_off+home_adv))/((h_off+home_adv)*0.424+0.548)*(AVG_BASE*0.424+0.548) + AVG_BASE)
		}
		max_error := 0.0
		var largest_k int
		for k, _ := range gamesCount {
			old_off := off_rating[k]
			//old_def := def_rating[k]
			off_rating[k] = adjusted_goals_scored[k] / gamesCount[k]
			def_rating[k] = adjusted_goals_allowed[k] / gamesCount[k]
			this_error := math.Sqrt((old_off - off_rating[k]) * (old_off - off_rating[k])) //+ math.Sqrt((old_def - def_rating[k])*(old_def - def_rating[k]))
			if this_error > max_error {
				max_error = this_error
				largest_k = k
			}
		}
		if max_error < 0.0001 {
			good_iterations += 1
		} else {
			good_iterations = 0
		}
		if good_iterations > 10 {
			log.Print(i)
			log.Printf("%d: %f", largest_k, max_error)
			log.Printf("%d %.6f %.6f", largest_k, off_rating[largest_k], def_rating[largest_k])
			log.Println("time per 100 iter 90k games:", time.Since(timer).Seconds()/float64(i)/float64(len(games))*100.0*90000)
			break
		}
		if i == 0 {
			log.Print(i)
			log.Printf("%d: %f", largest_k, max_error)
			log.Printf("%d %.6f %.6f %.6f", largest_k, off_rating[largest_k], def_rating[largest_k], gamesCount[largest_k])
		}
		if i%10 == 0 || i%10 == 1 {
			//      log.Print(i)
			//      log.Printf("%f: %f", largest_k, max_error)
			//      log.Printf("%f %.6f %.6f %.6f", largest_k, off_rating[largest_k], def_rating[largest_k], gamesCount[largest_k])
		}
	}
	new_ratings := make(map[int]*TeamRating)
	for k, _ := range ratings {
		index := table.Query(uint32(k))
		if gamesCount[index] == 0.0 {
			new_ratings[k] = nil
		} else {
			new_ratings[k] = new(TeamRating)
			new_ratings[k].Id = k
			new_ratings[k].Offense = lower_bound(off_rating[index], gamesCount[index])
			new_ratings[k].Defense = upper_bound(def_rating[index], gamesCount[index])
			new_ratings[k].Team = team_rating(new_ratings[k])
		}
	}
	log.Println(time.Unix(games[0].Timestamp, 0))
	return new_ratings
}

func lower_bound(avg float64, sample float64) float64 {
	return avg / (1.0 + math.Exp(-sample/4))
}

func upper_bound(avg float64, sample float64) float64 {
	return avg * (1.0 + math.Exp(-sample/4))
}

func team_weight(games float64) float64 {
	return 1/(1.0+math.Exp(-games/4))*2 - 1
}

func team_penalty(games float64) float64 {
	return 1/(1.0+math.Exp(-games/4))/2.5 + 0.6
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
	ratings := make(map[int]*TeamRating, len(v.Ratings))
	all_team_ids := make([]uint32, 0, len(v.Ratings))
	for _, v := range v.Ratings {
		ratings[v.Id] = v
		all_team_ids = append(all_team_ids, uint32(v.Id))
	}
	table := NewTable(all_team_ids)
	var q []*SpiGameType
	for i, _ := range v.Games {
		g := &v.Games[i]
		g.home_table_index = table.Query(uint32(g.Home_Id))
		g.away_table_index = table.Query(uint32(g.Away_Id))
		q = append(q, g)
	}
	ratings = spi(q, ratings, table)
	//  enc2 := json.NewEncoder(os.Stdout)
	//  err := enc2.Encode(ratings)
	//  if err != nil {
	//    panic(err)
	//  }
	enc := json.NewEncoder(c)
	err := enc.Encode(ratings)
	if err != nil {
		panic(err)
	}
}

func historicRatings(c http.ResponseWriter, req *http.Request) {
	log.Println("New Request")
	dec := json.NewDecoder(req.Body)
	var v GamesType
	if err := dec.Decode(&v); err != nil {
		fmt.Println(err)
		return
	}
	rps := v.historicRatings()
	enc := json.NewEncoder(c)
	enc.Encode(rps)
}

func evalPredictions(c http.ResponseWriter, req *http.Request) {
	log.Println("New Request")
	dec := json.NewDecoder(req.Body)
	var v GamesType
	if err := dec.Decode(&v); err != nil {
		fmt.Println(err)
		return
	}
	rps := v.eval()
	enc := json.NewEncoder(c)
	enc.Encode(rps)
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

type gameP struct {
	id        int
	date      time.Time
	homeScore int
	awayScore int
	homeAet   sql.NullInt32
	awayAet   sql.NullInt32
	homeField int
	homeId    int
	awayId    int
}

type teamP struct {
	offRating sql.NullFloat64
	defRating sql.NullFloat64
}

type goalP struct {
	playerId int
	teamId   int
	time     int
	penalty  bool
	ownGoal  bool
}

type playerP struct {
	playerId int
	teamId   int
	on       int
	off      int
	yellow   bool
	red      bool
	position sql.NullString
}

func loadGames() []*gameP {
	// Execute the query
	results, err := db.Query("SELECT `games`.`id`, `games`.`date`, `games`.`home_score`, `games`.`away_score`, `games`.`home_aet`, `games`.`away_aet`, `games`.`home_field`, `games`.`home_id`, `games`.`away_id` FROM `games` INNER JOIN `phases` ON `phases`.`id` = `games`.`phase_id` INNER JOIN `championships` ON `championships`.`id` = `phases`.`championship_id` WHERE (date > ?) AND `games`.`played` = 1 AND `championships`.`category_id` = 1", time.Now().Add(-time.Hour * 24 * 365 * 4))
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	var games []*gameP
	for results.Next() {
		var game gameP
		// for each row, scan the result into our tag composite object
		err = results.Scan(&game.id, &game.date, &game.homeScore, &game.awayScore, &game.homeAet, &game.awayAet, &game.homeField, &game.homeId, &game.awayId)
		if err != nil {
			panic(err.Error()) // proper error handling instead of panic in your app
		}
		games = append(games, &game)
	}

	return games
}

func loadTeams() map[int]*teamP {
	results, err := db.Query("SELECT id, off_rating, def_rating FROM teams")
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	teams := make(map[int]*teamP)
	for results.Next() {
		var team teamP
		var id int
		// for each row, scan the result into our tag composite object
		err = results.Scan(&id, &team.offRating, &team.defRating)
		if err != nil {
			panic(err.Error()) // proper error handling instead of panic in your app
		}
		teams[id] = &team
	}

	return teams
}

func loadGoals(games []*gameP) map[int][]*goalP {
	gameIds := make([]string, len(games))
	for i, g := range games {
		gameIds[i] = strconv.Itoa(g.id)
	}
	results, err := db.Query("SELECT player_id, game_id, team_id, `time`, penalty, own_goal from goals where game_id in (" + strings.Join(gameIds, ",") + ")")
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	goals := make(map[int][]*goalP)
	for results.Next() {
		var goal goalP
		var gameId int
		// for each row, scan the result into our tag composite object
		err = results.Scan(&goal.playerId, &gameId, &goal.teamId, &goal.time, &goal.penalty, &goal.ownGoal)
		if err != nil {
			panic(err.Error()) // proper error handling instead of panic in your app
		}
		goals[gameId] = append(goals[gameId], &goal)
	}

	return goals
}

func loadPlayers(games []*gameP) map[int][]*playerP {
	gameIds := make([]string, len(games))
	for i, g := range games {
		gameIds[i] = strconv.Itoa(g.id)
	}
	results, err := db.Query("select player_id, game_id, team_id, `on`, off, yellow, red, players.position from player_games INNER JOIN `players` ON `players`.`id` = `player_games`.`player_id` where off > 0 and game_id in (" + strings.Join(gameIds, ",") + ")")
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	players := make(map[int][]*playerP)
	for results.Next() {
		var player playerP
		var gameId int
		// for each row, scan the result into our tag composite object
		err = results.Scan(&player.playerId, &gameId, &player.teamId, &player.on, &player.off, &player.yellow, &player.red, &player.position)
		if err != nil {
			panic(err.Error()) // proper error handling instead of panic in your app
		}
		players[gameId] = append(players[gameId], &player)
	}

	return players
}

func filterPlayer(vs []*playerP, f func(p *playerP) bool) []*playerP {
	vsf := make([]*playerP, 0)
	for _, v := range vs {
		if f(v) {
			vsf = append(vsf, v)
		}
	}
	return vsf
}

func filterGoal(vs []*goalP, f func(p *goalP) bool) []*goalP {
	vsf := make([]*goalP, 0)
	for _, v := range vs {
		if f(v) {
			vsf = append(vsf, v)
		}
	}
	return vsf
}

func findIntervals(players []*playerP) []int {
	intervals := make(map[int]struct{})
	for _, player := range players {
		intervals[player.on] = struct{}{}
		intervals[player.off] = struct{}{}
	}
	ret := make([]int, 0, len(intervals))
	for k, _ := range intervals {
		ret = append(ret, k)
	}
	sort.Ints(ret)
	return ret
}

func playerRatings(c http.ResponseWriter, req *http.Request) {
	startFunc := time.Now()
	games := loadGames()
	log.Println(time.Since(startFunc))
	goals := loadGoals(games)
	log.Println(time.Since(startFunc))
	teams := loadTeams()
	log.Println(time.Since(startFunc))
	players := loadPlayers(games)
	log.Println(time.Since(startFunc))

	playerRatings := make(map[int]struct{
		off, def, minutes float64
	})

	now := time.Now()

	for _, game := range games {
		if len(players[game.id]) == 0 {
			continue
		}
		homeAdv := 0.0
		if game.homeField == 0 {
			homeAdv = HOME_ADV
		}
		if game.homeField == 2 {
			homeAdv = -HOME_ADV
		}

		weight := squash_date(float64(game.date.Unix()), float64(now.Unix()))
		length := 90.0
		if game.homeAet.Valid {
			length = 120
		}

		homeForZeroPer90 := -(teams[game.awayId].defRating.Float64+homeAdv)/((teams[game.awayId].defRating.Float64+homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548) / length / 11
		homeForGoalWeight := 1 / ((teams[game.awayId].defRating.Float64+homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548)
		homeAggZeroPer90 := (teams[game.awayId].offRating.Float64-homeAdv)/((teams[game.awayId].offRating.Float64-homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548) / length / 11
		homeAggGoalWeight := -1 / ((teams[game.awayId].offRating.Float64-homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548)

		awayForZeroPer90 := -(teams[game.homeId].defRating.Float64-homeAdv)/((teams[game.homeId].defRating.Float64-homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548) / length / 11
		awayForGoalWeight := 1 / ((teams[game.homeId].defRating.Float64-homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548)
		awayAggZeroPer90 := (teams[game.homeId].offRating.Float64+homeAdv)/((teams[game.homeId].offRating.Float64+homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548) / length / 11
		awayAggGoalWeight := -1 / ((teams[game.homeId].offRating.Float64+homeAdv)*0.424+0.548)*(AVG_BASE*0.424+0.548)

		homePlayers := filterPlayer(players[game.id], func(pg *playerP) bool { return pg.teamId == game.homeId })
		awayPlayers := filterPlayer(players[game.id], func(pg *playerP) bool { return pg.teamId == game.awayId })

		homeGoals := filterGoal(goals[game.id], func(goal *goalP) bool {
			return (goal.teamId == game.awayId && goal.ownGoal == true) || (goal.teamId == game.homeId && goal.ownGoal == false)
		})
		awayGoals := filterGoal(goals[game.id], func(goal *goalP) bool {
			return (goal.teamId == game.homeId && goal.ownGoal == true) || (goal.teamId == game.awayId && goal.ownGoal == false)
		})

		intervals := findIntervals(homePlayers)
		for i := 0; i < len(intervals) - 1; i++ {
			from := intervals[i]
			to := intervals[i+1]
			hp := filterPlayer(homePlayers, func(p *playerP) bool {
				return math.Max(float64(from), float64(p.on)) < math.Min(float64(to), float64(p.off))
			})
			pos := make(map[string]float64)
			for _, p := range hp {
				pos[p.position.String] += 1
			}
			offWIndividual := float64(len(hp)) / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""])
			offW := map[string]float64 {"g": offWIndividual * 0.3, "dc": offWIndividual * 0.7, "cm": offWIndividual, "fw": offWIndividual * 1.0, "": offWIndividual}
			defWIndividual := float64(len(hp)) / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""])
			defW := map[string]float64{ "g": defWIndividual * 4.0, "dc": defWIndividual * 2.0, "cm": defWIndividual, "fw": defWIndividual * 0.5, "": defWIndividual }
			homeGoalsInterval := filterGoal(homeGoals, func(g *goalP) bool {
				return g.time > from && g.time <= to
			})
			awayGoalsInterval := 0.0
			for _, g := range awayGoals {
				if g.time > from && g.time <= to {
					awayGoalsInterval += 1
				}
			}
			homeGoalsOwn := float64(0)
			for _, g := range homeGoalsInterval {
				if g.ownGoal {
					homeGoalsOwn += 1
				}
			}
			homeGoalsRegular := filterGoal(homeGoalsInterval, func(g *goalP) bool {
				return !g.ownGoal && !g.penalty
			})
			homeGoalsPenalty := filterGoal(homeGoalsInterval, func(g *goalP) bool {
				return !g.ownGoal && g.penalty
			})
			for _, v := range hp {
				playerRating := playerRatings[v.playerId]
				minutes := float64(to - from)
				playerRating.minutes += minutes * weight
				offPlayerWeight := offW[v.position.String]
				regularGoals := 0.0
				for _, g := range homeGoalsRegular {
					if g.playerId == v.playerId {
						regularGoals += 1
					}
				}
				penaltyGoals := 0.0
				for _, g := range homeGoalsPenalty {
					if g.playerId == v.playerId {
						penaltyGoals += 1
					}
				}
				playerRating.off += (minutes * homeForZeroPer90 * offPlayerWeight +
					homeGoalsOwn * homeForGoalWeight * offPlayerWeight / float64(len(hp)) +
					float64(len(homeGoalsRegular)) * homeForGoalWeight * offPlayerWeight / float64(len(hp)) / 4 * 3 +
					float64(len(homeGoalsPenalty)) * homeForGoalWeight * offPlayerWeight / float64(len(hp)) / 6 * 5 +
					regularGoals * homeForGoalWeight / 4 +
					penaltyGoals * homeForGoalWeight / 6) * weight
				playerRating.def += (minutes * homeAggZeroPer90 + awayGoalsInterval * homeAggGoalWeight / float64(len(hp))) * defW[v.position.String] * weight

				playerRatings[v.playerId] = playerRating
			}
		}

		intervals = findIntervals(awayPlayers)
		for i := 0; i < len(intervals) - 1; i++ {
			from := intervals[i]
			to := intervals[i+1]
			ap := filterPlayer(awayPlayers, func(p *playerP) bool {
				return math.Max(float64(from), float64(p.on)) < math.Min(float64(to), float64(p.off))
			})
			pos := make(map[string]float64)
			for _, p := range ap {
				pos[p.position.String] += 1
			}
			offWIndividual := float64(len(ap)) / (pos["g"] * 0.3 + pos["dc"] * 0.7 + pos["cm"] + pos["fw"] * 1.0 + pos[""])
			offW := map[string]float64 {"g": offWIndividual * 0.3, "dc": offWIndividual * 0.7, "cm": offWIndividual, "fw": offWIndividual * 1.0, "": offWIndividual}
			defWIndividual := float64(len(ap)) / (pos["g"] * 4.0 + pos["dc"] * 2.0 + pos["cm"] + pos["fw"] * 0.5 + pos[""])
			defW := map[string]float64{ "g": defWIndividual * 4.0, "dc": defWIndividual * 2.0, "cm": defWIndividual, "fw": defWIndividual * 0.5, "": defWIndividual }
			awayGoalsInterval := filterGoal(awayGoals, func(g *goalP) bool {
				return g.time > from && g.time <= to
			})
			homeGoalsInterval := 0.0
			for _, g := range homeGoals {
				if g.time > from && g.time <= to {
					homeGoalsInterval += 1
				}
			}
			awayGoalsOwn := float64(0)
			for _, g := range awayGoalsInterval {
				if g.ownGoal {
					awayGoalsOwn += 1
				}
			}
			awayGoalsRegular := filterGoal(awayGoalsInterval, func(g *goalP) bool {
				return !g.ownGoal && !g.penalty
			})
			awayGoalsPenalty := filterGoal(awayGoalsInterval, func(g *goalP) bool {
				return !g.ownGoal && g.penalty
			})
			for _, v := range ap {
				playerRating := playerRatings[v.playerId]
				minutes := float64(to - from)
				playerRating.minutes += minutes * weight
				offPlayerWeight := offW[v.position.String]
				regularGoals := 0.0
				for _, g := range awayGoalsRegular {
					if g.playerId == v.playerId {
						regularGoals += 1
					}
				}
				penaltyGoals := 0.0
				for _, g := range awayGoalsPenalty {
					if g.playerId == v.playerId {
						penaltyGoals += 1
					}
				}
				playerRating.off += (minutes * awayForZeroPer90 * offPlayerWeight +
					awayGoalsOwn * awayForGoalWeight * offPlayerWeight / float64(len(ap)) +
					float64(len(awayGoalsRegular)) * awayForGoalWeight* offPlayerWeight / float64(len(ap)) / 4 * 3 +
					float64(len(awayGoalsPenalty)) * awayForGoalWeight* offPlayerWeight / float64(len(ap)) / 6 * 5 +
					regularGoals *awayForGoalWeight/ 4 +
					penaltyGoals *awayForGoalWeight/ 6) * weight
				playerRating.def += (minutes * awayAggZeroPer90 + homeGoalsInterval * awayAggGoalWeight / float64(len(ap))) * defW[v.position.String] * weight

				playerRatings[v.playerId] = playerRating
			}
		}
	}
	log.Println(time.Since(startFunc))

	var insertSql strings.Builder
	insertSql.WriteString("INSERT INTO players (id,off_rating,def_rating,rating,created_at,updated_at) VALUES ")
	first := true
	for k, v := range playerRatings {
		if first {
			first = false
		} else {
			insertSql.WriteString(",")
		}
		insertSql.WriteString(fmt.Sprintf("(%d, %f, %f, %f, '%s', '%s')", k, v.off / v.minutes * 90, v.def / v.minutes * 90, (v.off + v.def) / (v.minutes + 900) * 90, now.Format("2006-01-02 15:04:05"), now.Format("2006-01-02 15:04:05")))
	}
	insertSql.WriteString(" ON DUPLICATE KEY UPDATE off_rating=VALUES(off_rating),def_rating=VALUES(def_rating),rating=VALUES(rating),updated_at=VALUES(updated_at)")
	log.Println(time.Since(startFunc))
	_, err := db.Exec(insertSql.String())
	if err != nil {
		panic(err.Error()) // proper error handling instead of panic in your app
	}
	log.Println(time.Since(startFunc))
}

var db, _ = sql.Open("mysql", "root@tcp(127.0.0.1:3306)/GolAberto_development?parseTime=true")

func main() {
	// defer the close till after the main function has finished
	// executing
	defer db.Close()

	fmt.Printf("Starting http Server ... ")
	http.Handle("/odds", http.HandlerFunc(calculateChampionshipOdds))
	http.Handle("/spi", http.HandlerFunc(calculatePowerRanking))
	http.Handle("/eval", http.HandlerFunc(evalPredictions))
	http.Handle("/historic_ratings", http.HandlerFunc(historicRatings))
	http.Handle("/player_ratings", http.HandlerFunc(playerRatings))
	log.Fatal(http.ListenAndServe(":6577", nil))
}
