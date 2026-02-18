class AddScrapeSinglePageToPhases < ActiveRecord::Migration[8.1]
  def change
    add_column :phases, :scrape_single_page, :boolean, default: false, null: false
  end
end
