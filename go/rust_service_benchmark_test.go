package main

// Opt-in HTTP benchmark host for the original handlers. Historical writes can
// only target a disposable benchmark database, never the application's database.
import (
	"database/sql"
	"io"
	"log"
	"net"
	"net/http"
	"os"
	"strings"
	"testing"
)

func TestRustServiceBenchmarkServer(t *testing.T) {
	address := os.Getenv("RUST_SERVICE_BENCHMARK_ADDRESS")
	if address == "" {
		t.Skip("optional HTTP performance benchmark")
	}
	log.SetOutput(io.Discard)
	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) { w.Write([]byte("ok")) })
	mux.HandleFunc("/odds", calculateChampionshipOdds)
	mux.HandleFunc("/spi", calculatePowerRanking)
	mux.HandleFunc("/eval", evalPredictions)
	if dsn := os.Getenv("RUST_SERVICE_BENCHMARK_DSN"); dsn != "" {
		// The benchmark script creates and drops this isolated schema.
		if !strings.Contains(dsn, "/golaberto_rust_bench_") {
			t.Fatal("historical benchmark requires a golaberto_rust_bench_ database")
		}
		var err error
		db, err = sql.Open("mysql", dsn)
		if err != nil {
			t.Fatal(err)
		}
		defer db.Close()
		mux.HandleFunc("/historic_ratings", historicRatings)
	}
	listener, err := net.Listen("tcp", address)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	if err := http.Serve(listener, mux); err != nil {
		t.Fatal(err)
	}
}
