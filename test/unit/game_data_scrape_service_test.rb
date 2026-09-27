require "test_helper"
require "scrape"
require "tmpdir"

class GameDataScrapeServiceTest < ActiveSupport::TestCase
  test "SofaScore requests use HTTPS and the Go fetcher" do
    requested_url = nil

    SofaScoreFetch.stub(:get, ->(url) {
      requested_url = url
      '{"events":[]}'
    }) do
      assert_equal({ "events" => [] }, ChampionshipGet.get("http://www.sofascore.com/api/v1/event/42/lineups"))
    end

    assert_equal "https://www.sofascore.com/api/v1/event/42/lineups", requested_url
  end

  test "SofaScore fetcher returns data and reports transport errors" do
    Dir.mktmpdir("fake-sofascore-fetch-") do |dir|
      fetcher = File.join(dir, "sofascore_fetch")
      File.write(fetcher, "#!/bin/sh\nprintf '{\"events\":[]}'\n")
      File.chmod(0755, fetcher)
      previous_fetcher = ENV["SOFASCORE_FETCH_BIN"]
      begin
        ENV["SOFASCORE_FETCH_BIN"] = fetcher
        assert_equal '{"events":[]}', SofaScoreFetch.get("https://www.sofascore.com/api/v1/event/42")

        File.write(fetcher, "#!/bin/sh\nprintf 'HTTP 403: Forbidden\\n' >&2\nexit 1\n")
        error = assert_raises(RuntimeError) { SofaScoreFetch.get("https://www.sofascore.com/api/v1/event/42") }
        assert_includes error.message, "HTTP 403: Forbidden"
      ensure
        ENV["SOFASCORE_FETCH_BIN"] = previous_fetcher
      end
    end
  end

  test "SofaScore error responses are logged for the phase" do
    url = "http://www.sofascore.com/api/v1/unique-tournament/390/season/89840/events/round/30"
    body = '{"error":{"code":403,"reason":"challenge"}}'
    phase = FakePhase.new(id: 42, name: "Liga", scrape_url: url.chomp("30"))
    errors = []

    Phase.stub(:find, phase) do
      ChampionshipGet.stub(:with_http_retries, body) do
        Rails.logger.stub(:error, ->(message) { errors << message }) do
          GameDataScrapeService.scrape_phase(phase, rounds: [30])
        end
      end
    end

    assert_equal 1, errors.size
    assert_includes errors.first, "ERROR phase #42"
    assert_includes errors.first, url
    assert_includes errors.first, "403"
    assert_includes errors.first, "challenge"
  end

  class FakePhase
    attr_reader :id, :name, :scrape_url, :scraped

    def initialize(id:, name:, scrape_url:, has_pending_games: true)
      @id = id
      @name = name
      @scrape_url = scrape_url
      @has_pending_games = has_pending_games
      @scraped = false
    end

    def mark_scraped!
      @scraped = true
    end
  end

  class FakePhaseRelation
    def initialize(phases)
      @phases = phases
    end

    def joins(*_args)
      self
    end

    def where(*_args)
      self
    end

    def not(*_args)
      self
    end

    def distinct
      @phases
    end
  end

  test "phases_to_scrape applies the recent pending games window by default" do
    where_calls = []
    fake_pending_scope = Object.new
    fake_pending_scope.define_singleton_method(:where) do |sql, value|
      where_calls << [sql, value]
      self
    end
    fake_pending_scope.define_singleton_method(:select) { |_field| self }
    fake_pending_scope.define_singleton_method(:distinct) { self }
    fake_pending_scope.define_singleton_method(:to_sql) { "SELECT DISTINCT phase_id FROM games" }

    fake_relation = FakePhaseRelation.new([])

    Game.stub(:where, fake_pending_scope) do
      Phase.stub(:joins, fake_relation) do
        GameDataScrapeService.phases_to_scrape
      end
    end

    assert where_calls.any? { |sql, _| sql.include?("date < ?") }
    assert where_calls.any? { |sql, _| sql.include?("date >= ?") }
  end

  test "phases_to_scrape can include all past pending games" do
    where_calls = []
    fake_pending_scope = Object.new
    fake_pending_scope.define_singleton_method(:where) do |sql, value|
      where_calls << [sql, value]
      self
    end
    fake_pending_scope.define_singleton_method(:select) { |_field| self }
    fake_pending_scope.define_singleton_method(:distinct) { self }
    fake_pending_scope.define_singleton_method(:to_sql) { "SELECT DISTINCT phase_id FROM games" }

    fake_relation = FakePhaseRelation.new([])

    Game.stub(:where, fake_pending_scope) do
      Phase.stub(:joins, fake_relation) do
        GameDataScrapeService.phases_to_scrape(include_all_past_unplayed: true)
      end
    end

    assert where_calls.any? { |sql, _| sql.include?("date < ?") }
    refute where_calls.any? { |sql, _| sql.include?("date >= ?") }
  end

  test "run_unlocked calls scrape for each phase with pending games" do
    phase = FakePhase.new(id: 42, name: "Liga", scrape_url: "http://sofascore.com/api/v1/unique-tournament/17/season/76986/events/round/", has_pending_games: true)

    mu = Mutex.new
    scraped_list = []
    fake_scrape = lambda { |phase_id, url|
      mu.synchronize { scraped_list << { phase_id: phase_id, url: url } }
    }

    GameDataScrapeService.stub(:phases_to_scrape, [phase]) do
      original_method = method(:scrape) rescue nil
      begin
        Object.send(:define_method, :scrape) { |pid, url, _opts = {}| fake_scrape.call(pid, url) }
        GameDataScrapeService.run_unlocked
      ensure
        if original_method
          Object.send(:define_method, :scrape, original_method)
        else
          Object.send(:remove_method, :scrape) rescue nil
        end
      end
    end

    assert_equal 1, scraped_list.size
    assert_equal 42, scraped_list[0][:phase_id]
    assert_equal "http://sofascore.com/api/v1/unique-tournament/17/season/76986/events/round/", scraped_list[0][:url]
  end

  test "scrape filters round events by the phase tournament IDs" do
    phase = Struct.new(:sofascore_tournament_ids) do
      def parsed_sofascore_tournament_ids
        [27214, 90333]
      end
    end.new("27214,90333")
    data = {
      "events" => [
        { "tournament" => { "id" => 27214 } },
        { "tournament" => { "id" => 12345 } },
        { "tournament" => { "id" => 90333 } },
      ],
    }
    parsed_tournament_ids = []

    original_method = method(:parse_match) rescue nil
    begin
      Object.send(:define_method, :parse_match) do |_phase, _data, match, rounds, _create_groups, _refetch|
        parsed_tournament_ids << [match.dig("tournament", "id"), rounds]
      end
      Phase.stub(:find, phase) do
        ChampionshipGet.stub(:get, data) do
          scrape(42, "http://example.com/events/round/", rounds: [2])
        end
      end
    ensure
      if original_method
        Object.send(:define_method, :parse_match, original_method)
      else
        Object.send(:remove_method, :parse_match) rescue nil
      end
    end

    assert_equal [[27214, [2]], [90333, [2]]], parsed_tournament_ids
  end

  test "scrape accepts every event when the phase has no tournament filter" do
    phase = Struct.new(:sofascore_tournament_ids) do
      def parsed_sofascore_tournament_ids
        []
      end
    end.new(nil)
    events = [
      { "tournament" => { "id" => 27214 } },
      { "tournament" => { "id" => 12345 } },
    ]

    assert_equal events, scrape_events_for_phase(phase, { "events" => events })
  end

  test "run_unlocked handles scrape errors gracefully and continues" do
    phase1 = FakePhase.new(id: 1, name: "Phase1", scrape_url: "http://example.com/1/", has_pending_games: true)
    phase2 = FakePhase.new(id: 2, name: "Phase2", scrape_url: "http://example.com/2/", has_pending_games: true)

    mu = Mutex.new
    scraped_list = []
    fake_scrape = lambda { |phase_id, url|
      raise "network timeout" if phase_id == 1
      mu.synchronize { scraped_list << { phase_id: phase_id, url: url } }
    }

    GameDataScrapeService.stub(:phases_to_scrape, [phase1, phase2]) do
      original_method = method(:scrape) rescue nil
      begin
        Object.send(:define_method, :scrape) { |pid, url, _opts = {}| fake_scrape.call(pid, url) }
        GameDataScrapeService.run_unlocked
      ensure
        if original_method
          Object.send(:define_method, :scrape, original_method)
        else
          Object.send(:remove_method, :scrape) rescue nil
        end
      end
    end

    assert_equal 1, scraped_list.size
    assert_equal 2, scraped_list[0][:phase_id]
  end

  test "run_unlocked scrapes all phases with at most MAX_CONCURRENCY threads" do
    phases = 6.times.map { |i| FakePhase.new(id: i + 1, name: "Phase#{i + 1}", scrape_url: "http://example.com/#{i + 1}/") }

    mu = Mutex.new
    scraped_ids = []
    max_concurrent = 0
    current_concurrent = 0

    fake_scrape = lambda { |phase_id, _url|
      mu.synchronize do
        current_concurrent += 1
        max_concurrent = [max_concurrent, current_concurrent].max
      end
      sleep 0.05
      mu.synchronize do
        scraped_ids << phase_id
        current_concurrent -= 1
      end
    }

    GameDataScrapeService.stub(:phases_to_scrape, phases) do
      original_method = method(:scrape) rescue nil
      begin
        Object.send(:define_method, :scrape) { |pid, url, _opts = {}| fake_scrape.call(pid, url) }
        GameDataScrapeService.run_unlocked
      ensure
        if original_method
          Object.send(:define_method, :scrape, original_method)
        else
          Object.send(:remove_method, :scrape) rescue nil
        end
      end
    end

    assert_equal 6, scraped_ids.size, "All 6 phases should be scraped"
    assert_equal (1..6).to_a, scraped_ids.sort
    assert max_concurrent <= GameDataScrapeService::MAX_CONCURRENCY,
      "Expected at most #{GameDataScrapeService::MAX_CONCURRENCY} concurrent scrapes, got #{max_concurrent}"
    assert max_concurrent > 1,
      "Expected parallel execution (got max concurrency of #{max_concurrent})"
  end

  test "run raises when already locked" do
    lock_path = GameDataScrapeService::LOCK_PATH
    FileUtils.mkdir_p(File.dirname(lock_path))
    lock_file = File.open(lock_path, "w")
    lock_file.flock(File::LOCK_EX)

    begin
      assert_raises(RuntimeError, /already running/) do
        GameDataScrapeService.run
      end
    ensure
      lock_file.flock(File::LOCK_UN)
      lock_file.close
    end
  end

  test "start_async returns busy when already locked" do
    lock_path = GameDataScrapeService::LOCK_PATH
    FileUtils.mkdir_p(File.dirname(lock_path))
    lock_file = File.open(lock_path, "w")
    lock_file.flock(File::LOCK_EX)

    begin
      result = GameDataScrapeService.start_async
      assert_equal :busy, result
    ensure
      lock_file.flock(File::LOCK_UN)
      lock_file.close
    end
  end
end
