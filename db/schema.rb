# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# This file is the source Rails uses to define your schema when running `bin/rails
# db:schema:load`. When creating a new database, `bin/rails db:schema:load` tends to
# be faster and is potentially less error prone than running all of your
# migrations from scratch. Old migrations may fail to apply correctly if those
# migrations use external dependencies or application code.
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema[8.1].define(version: 2026_03_30_000000) do
  create_table "categories", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "name"
  end

  create_table "championships", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.date "begin", null: false
    t.integer "category_id", default: 0, null: false
    t.datetime "created_at", precision: nil, null: false
    t.date "end", null: false
    t.string "name", default: "", null: false
    t.integer "point_draw", default: 1, null: false
    t.integer "point_loss", default: 0, null: false
    t.integer "point_win", default: 3, null: false
    t.integer "region", default: 0, null: false
    t.text "region_name"
    t.boolean "show_country", default: false, null: false
    t.datetime "updated_at", precision: nil, null: false
  end

  create_table "collation_test", id: false, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", options: "ENGINE=MyISAM", force: :cascade do |t|
    t.string "name", null: false
  end

  create_table "comments", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.text "comment"
    t.integer "commentable_id", default: 0, null: false
    t.string "commentable_type", limit: 15, default: "", null: false
    t.datetime "created_at", precision: nil, null: false
    t.string "title", limit: 50, default: ""
    t.integer "user_id", default: 0, null: false
    t.index ["user_id"], name: "fk_comments_user"
  end

  create_table "delayed_jobs", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "attempts", default: 0
    t.datetime "created_at", precision: nil
    t.datetime "failed_at", precision: nil
    t.text "handler"
    t.string "last_error"
    t.datetime "locked_at", precision: nil
    t.string "locked_by"
    t.integer "priority", default: 0
    t.string "queue"
    t.datetime "run_at", precision: nil
    t.datetime "updated_at", precision: nil
  end

  create_table "game_goals_versions", id: false, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "game_version_id", default: 0, null: false
    t.integer "goal_id", default: 0, null: false
  end

  create_table "game_versions", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "attendance"
    t.integer "away_aet"
    t.integer "away_id", default: 0
    t.float "away_importance"
    t.integer "away_pen"
    t.integer "away_score", default: 0
    t.datetime "date", precision: nil, null: false
    t.integer "game_id"
    t.boolean "has_time", default: false
    t.integer "home_aet"
    t.integer "home_field", default: 0, null: false
    t.integer "home_id", default: 0
    t.float "home_importance"
    t.integer "home_pen"
    t.integer "home_score", default: 0
    t.integer "phase_id"
    t.boolean "played", default: false
    t.integer "referee_id"
    t.integer "round"
    t.string "soccerway_id"
    t.string "sofascore_id"
    t.integer "stadium_id"
    t.datetime "updated_at", precision: nil, null: false
    t.integer "updater_id", default: 0, null: false
    t.integer "version"
    t.index ["game_id"], name: "index_game_versions_on_game_id"
    t.index ["updater_id"], name: "index_game_versions_on_updater_id"
  end

  create_table "games", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "attendance"
    t.integer "away_aet"
    t.integer "away_id", default: 0, null: false
    t.float "away_importance"
    t.integer "away_pen"
    t.integer "away_score", default: 0, null: false
    t.integer "championship_category_id", default: 0, null: false
    t.datetime "date", precision: nil, null: false
    t.boolean "has_time", default: false
    t.integer "home_aet"
    t.integer "home_field", default: 0, null: false
    t.integer "home_id", default: 0, null: false
    t.float "home_importance"
    t.integer "home_pen"
    t.integer "home_score", default: 0, null: false
    t.integer "phase_id"
    t.boolean "played", default: false, null: false
    t.integer "referee_id"
    t.integer "round"
    t.string "soccerway_id"
    t.string "sofascore_id"
    t.integer "stadium_id"
    t.datetime "updated_at", precision: nil, null: false
    t.integer "updater_id", default: 0, null: false
    t.integer "version"
    t.index ["away_id"], name: "index_games_on_away_id"
    t.index ["championship_category_id", "played", "date", "phase_id"], name: "index_games_on_category_played_date_phase"
    t.index ["date"], name: "index_games_on_date"
    t.index ["home_id"], name: "index_games_on_home_id"
    t.index ["phase_id"], name: "index_games_on_phase_id"
    t.index ["played"], name: "index_games_on_played"
    t.index ["referee_id"], name: "index_games_on_referee_id"
    t.index ["soccerway_id"], name: "index_games_on_soccerway_id", unique: true
    t.index ["stadium_id"], name: "index_games_on_stadium_id"
    t.index ["updated_at", "updater_id"], name: "index_games_on_updated_at_and_updater_id"
  end

  create_table "goals", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.boolean "aet", default: false, null: false
    t.datetime "created_at", precision: nil, null: false
    t.integer "game_id", default: 0
    t.boolean "own_goal", default: false, null: false
    t.boolean "penalty", default: false, null: false
    t.integer "player_id", default: 0, null: false
    t.integer "team_id", default: 0, null: false
    t.integer "time", default: 0, null: false
    t.datetime "updated_at", precision: nil, null: false
    t.index ["game_id"], name: "index_goals_on_game_id"
    t.index ["player_id", "game_id"], name: "player"
  end

  create_table "groups", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.datetime "created_at", precision: nil, null: false
    t.string "name", default: "", null: false
    t.integer "odds_progress"
    t.integer "phase_id", default: 0, null: false
    t.datetime "updated_at", precision: nil, null: false
    t.text "zones"
    t.index ["phase_id"], name: "index_groups_on_phase_id"
  end

  create_table "historical_ratings", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.float "def_rating", null: false
    t.date "measure_date", null: false
    t.float "off_rating", null: false
    t.float "rating", null: false
    t.integer "team_id", null: false
    t.index ["team_id", "measure_date"], name: "index_historical_ratings_on_team_id_and_measure_date", unique: true
    t.index ["team_id"], name: "index_historical_ratings_on_team_id"
  end

  create_table "open_id_authentication_associations", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "assoc_type"
    t.string "handle"
    t.integer "issued"
    t.integer "lifetime"
    t.binary "secret"
    t.binary "server_url"
  end

  create_table "open_id_authentication_nonces", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "salt", null: false
    t.string "server_url"
    t.integer "timestamp", null: false
  end

  create_table "phases", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "bonus_points", default: 0, null: false
    t.integer "bonus_points_threshold", default: 0, null: false
    t.integer "championship_id", default: 0, null: false
    t.datetime "created_at", precision: nil, null: false
    t.string "name", default: "", null: false
    t.integer "order_by", default: 0, null: false
    t.string "scrape_url"
    t.string "sort", default: "pt, w, gd, gf, gp, g_away, name", null: false
    t.datetime "updated_at", precision: nil, null: false
    t.index ["championship_id"], name: "championship"
  end

  create_table "player_games", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.float "def_rating"
    t.integer "game_id", null: false
    t.integer "off", default: 0, null: false
    t.float "off_rating"
    t.integer "on", default: 0, null: false
    t.integer "player_id", default: 0, null: false
    t.boolean "red", default: false, null: false
    t.integer "team_id", default: 0, null: false
    t.boolean "yellow", default: false, null: false
    t.index ["game_id", "team_id", "player_id"], name: "index_player_games_on_game_id_and_team_id_and_player_id", unique: true
    t.index ["player_id"], name: "index_player_games_on_player_id"
  end

  create_table "players", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.date "birth"
    t.string "country"
    t.datetime "created_at", precision: nil, null: false
    t.float "def_rating"
    t.string "full_name"
    t.integer "height"
    t.string "name", default: "", null: false
    t.float "off_rating"
    t.string "position", limit: 3
    t.float "rating"
    t.string "soccerway_id"
    t.string "sofascore_id"
    t.datetime "updated_at", precision: nil, null: false
    t.index ["rating"], name: "index_players_on_rating"
    t.index ["soccerway_id"], name: "index_players_on_soccerway_id", unique: true
  end

  create_table "referee_champs", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "championship_id", default: 0, null: false
    t.integer "referee_id", default: 0, null: false
  end

  create_table "referees", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.datetime "created_at", precision: nil, null: false
    t.string "location"
    t.string "name", default: "", null: false
    t.datetime "updated_at", precision: nil, null: false
  end

  create_table "roles", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "name"
  end

  create_table "roles_users", id: false, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "role_id"
    t.integer "user_id"
    t.index ["role_id"], name: "index_roles_users_on_role_id"
    t.index ["user_id"], name: "index_roles_users_on_user_id"
  end

  create_table "stadia", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "city"
    t.string "country"
    t.datetime "created_at", precision: nil, null: false
    t.string "full_name"
    t.string "name", default: "", null: false
    t.datetime "updated_at", precision: nil, null: false
  end

  create_table "team_geocodes", charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.text "data", size: :long, collation: "utf8mb4_bin"
    t.integer "team_id", null: false
    t.index ["team_id"], name: "index_team_geocodes_on_team_id"
    t.check_constraint "json_valid(`data`)", name: "data"
  end

  create_table "team_group_odds_histories", charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.datetime "captured_at", null: false
    t.datetime "created_at", null: false
    t.text "odds", null: false
    t.date "recorded_on", null: false
    t.integer "team_group_id", null: false
    t.datetime "updated_at", null: false
    t.index ["team_group_id", "recorded_on"], name: "index_tg_odds_histories_on_team_group_and_day", unique: true
  end

  create_table "team_groups", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "add_sub", default: 0, null: false
    t.integer "bias", default: 0, null: false
    t.text "comment"
    t.datetime "created_at", precision: nil, null: false
    t.integer "group_id", default: 0, null: false
    t.text "odds"
    t.integer "team_id", default: 0, null: false
    t.datetime "updated_at", precision: nil, null: false
    t.index ["group_id", "team_id"], name: "group", unique: true
    t.index ["id"], name: "id"
  end

  create_table "team_players", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.integer "championship_id", default: 0, null: false
    t.datetime "created_at", precision: nil, null: false
    t.integer "player_id", default: 0, null: false
    t.integer "team_id", default: 0, null: false
    t.datetime "updated_at", precision: nil, null: false
  end

  create_table "teams", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.string "city"
    t.string "country", default: "", null: false
    t.datetime "created_at", precision: nil, null: false
    t.float "def_rating"
    t.date "foundation"
    t.string "full_name"
    t.string "legacy_logo"
    t.string "logo_content_type"
    t.string "logo_file_name"
    t.integer "logo_file_size"
    t.datetime "logo_updated_at", precision: nil
    t.string "name", default: "", null: false
    t.float "off_rating"
    t.float "rating"
    t.integer "stadium_id"
    t.integer "team_type", default: 0, null: false
    t.datetime "updated_at", precision: nil, null: false
  end

  create_table "users", id: :integer, charset: "utf8mb3", collation: "utf8mb3_unicode_ci", force: :cascade do |t|
    t.text "about_me"
    t.string "avatar_content_type"
    t.string "avatar_file_name"
    t.integer "avatar_file_size"
    t.datetime "avatar_updated_at", precision: nil
    t.date "birthday"
    t.datetime "created_at", precision: nil
    t.string "crypted_password", limit: 40
    t.string "email"
    t.string "identity_url"
    t.datetime "last_login", precision: nil
    t.string "location", limit: 100
    t.string "login"
    t.string "name", limit: 100
    t.string "openid_connect_token"
    t.string "remember_token"
    t.datetime "remember_token_expires_at", precision: nil
    t.string "salt", limit: 40
    t.datetime "updated_at", precision: nil
  end

  add_foreign_key "historical_ratings", "teams"
  add_foreign_key "player_games", "games", on_delete: :cascade
  add_foreign_key "team_geocodes", "teams"
  add_foreign_key "team_group_odds_histories", "team_groups"
end
