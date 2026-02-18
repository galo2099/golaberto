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
    attr_reader :id, :name, :scrape_url, :scraped

    def initialize(id:, name:, scrape_url:, has_pending_games: true)
      @id = id
      @name = name
      @scrape_url = scrape_url
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

    scraped_calls = []
    fake_scrape = lambda { |phase_id, url|
      scraped_calls << { phase_id: phase_id, url: url }
    }

    GameDataScrapeService.stub(:phases_to_scrape, [phase]) do
      # Redefine scrape at top-level for the duration of the test
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

    assert_equal 1, scraped_calls.size
    assert_equal 42, scraped_calls[0][:phase_id]
    assert_equal "http://sofascore.com/api/v1/unique-tournament/17/season/76986/events/round/", scraped_calls[0][:url]
  end

  test "run_unlocked handles scrape errors gracefully and continues" do
    phase1 = FakePhase.new(id: 1, name: "Phase1", scrape_url: "http://example.com/1/", has_pending_games: true)
    phase2 = FakePhase.new(id: 2, name: "Phase2", scrape_url: "http://example.com/2/", has_pending_games: true)

    scraped_calls = []
    fake_scrape = lambda { |phase_id, url|
      raise "network timeout" if phase_id == 1
      scraped_calls << { phase_id: phase_id, url: url }
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

    assert_equal 1, scraped_calls.size
    assert_equal 2, scraped_calls[0][:phase_id]
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
