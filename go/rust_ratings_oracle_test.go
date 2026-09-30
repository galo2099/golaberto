package main

// Offline differential oracle. Never writes persistent tables: the historical
// endpoint is directed to a connection-local temporary table.
import (
	"encoding/json"
	"net/http/httptest"
	"os"
	"strings"
	"testing"
)

func TestRustRatingsOracle(t *testing.T) {
	input := os.Getenv("RUST_RATINGS_ORACLE_INPUT")
	if input == "" {
		t.Skip("set RUST_RATINGS_ORACLE_INPUT, OUTPUT and MODE for a differential run")
	}
	bytes, err := os.ReadFile(input)
	if err != nil {
		t.Fatal(err)
	}
	var request GamesType
	if err = json.Unmarshal(bytes, &request); err != nil {
		t.Fatal(err)
	}
	var result interface{}
	switch os.Getenv("RUST_RATINGS_ORACLE_MODE") {
	case "spi":
		response := httptest.NewRecorder()
		calculatePowerRanking(response, httptest.NewRequest("POST", "/spi", strings.NewReader(string(bytes))))
		if err = json.Unmarshal(response.Body.Bytes(), &result); err != nil {
			t.Fatal(err)
		}
	case "eval":
		result = request.eval()
	case "historic":
		db.SetMaxOpenConns(1)
		_, err = db.Exec("CREATE TEMPORARY TABLE historical_ratings (team_id INT NOT NULL, off_rating DOUBLE NOT NULL, def_rating DOUBLE NOT NULL, rating DOUBLE NOT NULL, measure_date DATE NOT NULL, UNIQUE KEY (team_id,measure_date))")
		if err != nil {
			t.Fatal(err)
		}
		defer db.Exec("DROP TEMPORARY TABLE historical_ratings")
		request.historicRatings()
		rows, err := db.Query("SELECT team_id,off_rating,def_rating,rating,DATE_FORMAT(measure_date,'%Y-%m-%d') FROM historical_ratings ORDER BY team_id,measure_date")
		if err != nil {
			t.Fatal(err)
		}
		defer rows.Close()
		values := []map[string]interface{}{}
		for rows.Next() {
			var id int
			var off, def, rating float64
			var date string
			if err := rows.Scan(&id, &off, &def, &rating, &date); err != nil {
				t.Fatal(err)
			}
			values = append(values, map[string]interface{}{"team_id": id, "off_rating": off, "def_rating": def, "rating": rating, "measure_date": date})
		}
		if err := rows.Err(); err != nil {
			t.Fatal(err)
		}
		result = values
	default:
		t.Fatal("unknown oracle mode")
	}
	encoded, err := json.Marshal(result)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(os.Getenv("RUST_RATINGS_ORACLE_OUTPUT"), encoded, 0600); err != nil {
		t.Fatal(err)
	}
}
