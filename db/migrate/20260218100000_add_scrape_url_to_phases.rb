class AddScrapeUrlToPhases < ActiveRecord::Migration[8.1]
  def change
    add_column :phases, :scrape_url, :string
  end
end
