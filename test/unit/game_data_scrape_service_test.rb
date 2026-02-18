require "test_helper"
require "scrape"

class GameDataScrapeServiceTest < ActiveSupport::TestCase
  class FakePendingGames
    def initialize(has_pending)
      @has_pending = has_pending
    end

    def where(*_args)
      self
    end

    def exists?
      @has_pending
    end
  end

  class FakePhase
    attr_reader :id, :name, :scrape_url, :scrape_single_page, :scraped

    def initialize(id:, name:, scrape_url:, has_pending_games: true, scrape_single_page: false)
      @id = id
      @name = name
      @scrape_url = scrape_url
      @scrape_single_page = scrape_single_page
      @has_pending_games = has_pending_games
      @scraped = false
    end

    def games
      FakePendingGames.new(@has_pending_games)
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

  test "phases_to_scrape returns only phases with scrape_url and pending games" do
    with_pending = FakePhase.new(id: 1, name: "Group Stage", scrape_url: "http://sofascore.com/api/v1/events/round/", has_pending_games: true)
    no_pending = FakePhase.new(id: 2, name: "Knockout", scrape_url: "http://sofascore.com/api/v1/events/round/", has_pending_games: false)

    phases = [with_pending, no_pending]
    fake_relation = FakePhaseRelation.new(phases)

    result = Phase.stub(:joins, fake_relation) do
      GameDataScrapeService.phases_to_scrape
    end

    assert_includes result, with_pending
    refute_includes result, no_pending
  end

  test "run_unlocked calls scrape for each phase with pending games" do
    phase = FakePhase.new(id: 42, name: "Liga", scrape_url: "http://sofascore.com/api/v1/unique-tournament/17/season/76986/events/round/", has_pending_games: true)

    mu = Mutex.new
    scraped_list = []
    fake_scrape = lambda { |phase_id, url, opts|
      mu.synchronize { scraped_list << { phase_id: phase_id, url: url, opts: opts } }
    }

    GameDataScrapeService.stub(:phases_to_scrape, [phase]) do
      original_method = method(:scrape) rescue nil
      begin
        Object.send(:define_method, :scrape) { |pid, url, opts = {}| fake_scrape.call(pid, url, opts) }
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
    assert_empty scraped_list[0][:opts]
  end

  test "run_unlocked passes single_page option for single-page phases" do
    per_round = FakePhase.new(id: 1, name: "League", scrape_url: "http://example.com/round/", scrape_single_page: false)
    single_pg = FakePhase.new(id: 2, name: "Qualification", scrape_url: "http://example.com/slug/qual", scrape_single_page: true)

    mu = Mutex.new
    scraped_list = []
    fake_scrape = lambda { |phase_id, url, opts|
      mu.synchronize { scraped_list << { phase_id: phase_id, url: url, opts: opts } }
    }

    GameDataScrapeService.stub(:phases_to_scrape, [per_round, single_pg]) do
      original_method = method(:scrape) rescue nil
      begin
        Object.send(:define_method, :scrape) { |pid, url, opts = {}| fake_scrape.call(pid, url, opts) }
        GameDataScrapeService.run_unlocked
      ensure
        if original_method
          Object.send(:define_method, :scrape, original_method)
        else
          Object.send(:remove_method, :scrape) rescue nil
        end
      end
    end

    assert_equal 2, scraped_list.size
    round_call = scraped_list.find { |c| c[:phase_id] == 1 }
    single_call = scraped_list.find { |c| c[:phase_id] == 2 }

    assert_empty round_call[:opts], "Per-round phase should not pass single_page"
    assert_equal({ single_page: true }, single_call[:opts], "Single-page phase should pass single_page: true")
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
