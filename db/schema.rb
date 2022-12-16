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

ActiveRecord::Schema.define(version: 2022_12_16_213315) do

  create_table "categories", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name"
  end

  create_table "championships", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name", default: "", null: false
    t.date "begin", null: false
    t.date "end", null: false
    t.integer "point_win", default: 3, null: false
    t.integer "point_draw", default: 1, null: false
    t.integer "point_loss", default: 0, null: false
    t.integer "category_id", default: 0, null: false
    t.boolean "show_country", default: false, null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.integer "region", default: 0, null: false
    t.text "region_name"
  end

  create_table "collation_test", id: false, charset: "utf8", collation: "utf8_unicode_ci", options: "ENGINE=MyISAM", force: :cascade do |t|
    t.string "name", null: false
  end

  create_table "comments", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "title", limit: 50, default: ""
    t.text "comment"
    t.datetime "created_at", null: false
    t.integer "commentable_id", default: 0, null: false
    t.string "commentable_type", limit: 15, default: "", null: false
    t.integer "user_id", default: 0, null: false
    t.index ["user_id"], name: "fk_comments_user"
  end

  create_table "delayed_jobs", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "priority", default: 0
    t.integer "attempts", default: 0
    t.text "handler"
    t.string "last_error"
    t.datetime "run_at"
    t.datetime "locked_at"
    t.datetime "failed_at"
    t.string "locked_by"
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string "queue"
  end

  create_table "game_goals_versions", id: false, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "game_version_id", default: 0, null: false
    t.integer "goal_id", default: 0, null: false
  end

  create_table "game_versions", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "game_id"
    t.integer "version"
    t.integer "home_id", default: 0
    t.integer "away_id", default: 0
    t.integer "phase_id"
    t.integer "round"
    t.integer "attendance"
    t.integer "stadium_id"
    t.integer "referee_id"
    t.integer "home_score", default: 0
    t.integer "away_score", default: 0
    t.integer "home_pen"
    t.integer "away_pen"
    t.boolean "played", default: false
    t.integer "updater_id", default: 0, null: false
    t.datetime "updated_at", null: false
    t.integer "home_aet"
    t.integer "away_aet"
    t.datetime "date", null: false
    t.boolean "has_time", default: false
    t.integer "home_field", default: 0, null: false
    t.float "home_importance"
    t.float "away_importance"
    t.string "soccerway_id"
    t.index ["game_id"], name: "index_game_versions_on_game_id"
    t.index ["updater_id"], name: "index_game_versions_on_updater_id"
  end

  create_table "games", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "home_id", default: 0, null: false
    t.integer "away_id", default: 0, null: false
    t.integer "phase_id"
    t.integer "round"
    t.integer "attendance"
    t.integer "stadium_id"
    t.integer "referee_id"
    t.integer "home_score", default: 0, null: false
    t.integer "away_score", default: 0, null: false
    t.integer "home_pen"
    t.integer "away_pen"
    t.boolean "played", default: false, null: false
    t.integer "version"
    t.integer "updater_id", default: 0, null: false
    t.datetime "updated_at", null: false
    t.integer "home_aet"
    t.integer "away_aet"
    t.datetime "date", null: false
    t.boolean "has_time", default: false
    t.integer "home_field", default: 0, null: false
    t.float "home_importance"
    t.float "away_importance"
    t.string "soccerway_id"
    t.index ["away_id"], name: "index_games_on_away_id"
    t.index ["date"], name: "index_games_on_date"
    t.index ["home_id"], name: "index_games_on_home_id"
    t.index ["phase_id"], name: "index_games_on_phase_id"
    t.index ["played"], name: "index_games_on_played"
    t.index ["referee_id"], name: "index_games_on_referee_id"
    t.index ["soccerway_id"], name: "index_games_on_soccerway_id", unique: true
    t.index ["stadium_id"], name: "index_games_on_stadium_id"
    t.index ["updated_at", "updater_id"], name: "index_games_on_updated_at_and_updater_id"
  end

  create_table "goals", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "player_id", default: 0, null: false
    t.integer "game_id", default: 0
    t.integer "team_id", default: 0, null: false
    t.integer "time", default: 0, null: false
    t.boolean "penalty", default: false, null: false
    t.boolean "own_goal", default: false, null: false
    t.boolean "aet", default: false, null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["game_id"], name: "index_goals_on_game_id"
    t.index ["player_id", "game_id"], name: "player"
  end

  create_table "groups", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "phase_id", default: 0, null: false
    t.string "name", default: "", null: false
    t.integer "promoted", default: 0, null: false
    t.integer "relegated", default: 0, null: false
    t.integer "odds_progress"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.text "zones"
  end

  create_table "historical_ratings", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "team_id", null: false
    t.float "rating", null: false
    t.date "measure_date", null: false
    t.float "off_rating", null: false
    t.float "def_rating", null: false
    t.index ["team_id", "measure_date"], name: "index_historical_ratings_on_team_id_and_measure_date", unique: true
    t.index ["team_id"], name: "index_historical_ratings_on_team_id"
  end

  create_table "open_id_authentication_associations", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "issued"
    t.integer "lifetime"
    t.string "handle"
    t.string "assoc_type"
    t.binary "server_url"
    t.binary "secret"
  end

  create_table "open_id_authentication_nonces", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "timestamp", null: false
    t.string "server_url"
    t.string "salt", null: false
  end

  create_table "phases", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "championship_id", default: 0, null: false
    t.string "name", default: "", null: false
    t.integer "order_by", default: 0, null: false
    t.string "sort", default: "pt, w, gd, gf, gp, g_away, name", null: false
    t.integer "bonus_points", default: 0, null: false
    t.integer "bonus_points_threshold", default: 0, null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.index ["championship_id"], name: "championship"
  end

  create_table "player_games", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "player_id", default: 0, null: false
    t.integer "game_id", default: 0, null: false
    t.integer "team_id", default: 0, null: false
    t.integer "on", default: 0, null: false
    t.integer "off", default: 0, null: false
    t.boolean "yellow", default: false, null: false
    t.boolean "red", default: false, null: false
    t.index ["game_id", "team_id", "player_id"], name: "index_player_games_on_game_id_and_team_id_and_player_id", unique: true
    t.index ["player_id"], name: "index_player_games_on_player_id"
  end

  create_table "players", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name", default: "", null: false
    t.string "position", limit: 3
    t.date "birth"
    t.string "country"
    t.string "full_name"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.string "soccerway_id"
    t.float "rating"
    t.float "off_rating"
    t.float "def_rating"
    t.index ["rating"], name: "index_players_on_rating"
    t.index ["soccerway_id"], name: "index_players_on_soccerway_id", unique: true
  end

  create_table "referee_champs", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "referee_id", default: 0, null: false
    t.integer "championship_id", default: 0, null: false
  end

  create_table "referees", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name", default: "", null: false
    t.string "location"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
  end

  create_table "roles", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name"
  end

  create_table "roles_users", id: false, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "role_id"
    t.integer "user_id"
    t.index ["role_id"], name: "index_roles_users_on_role_id"
    t.index ["user_id"], name: "index_roles_users_on_user_id"
  end

  create_table "stadia", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name", default: "", null: false
    t.string "full_name"
    t.string "city"
    t.string "country"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
  end

  create_table "team_groups", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "group_id", default: 0, null: false
    t.integer "team_id", default: 0, null: false
    t.integer "add_sub", default: 0, null: false
    t.integer "bias", default: 0, null: false
    t.text "comment"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.text "odds"
    t.index ["group_id", "team_id"], name: "group", unique: true
    t.index ["id"], name: "id"
  end

  create_table "team_players", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.integer "team_id", default: 0, null: false
    t.integer "player_id", default: 0, null: false
    t.integer "championship_id", default: 0, null: false
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
  end

  create_table "teams", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "name", default: "", null: false
    t.string "country", default: "", null: false
    t.string "legacy_logo"
    t.string "city"
    t.integer "stadium_id"
    t.date "foundation"
    t.string "full_name"
    t.string "logo_file_name"
    t.string "logo_content_type"
    t.integer "logo_file_size"
    t.datetime "logo_updated_at"
    t.datetime "created_at", null: false
    t.datetime "updated_at", null: false
    t.float "rating"
    t.float "off_rating"
    t.float "def_rating"
    t.integer "team_type", default: 0, null: false
  end

  create_table "users", id: :integer, charset: "utf8", collation: "utf8_unicode_ci", force: :cascade do |t|
    t.string "login"
    t.string "email"
    t.string "crypted_password", limit: 40
    t.string "salt", limit: 40
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string "remember_token"
    t.datetime "remember_token_expires_at"
    t.string "identity_url"
    t.string "name", limit: 30
    t.string "location", limit: 100
    t.date "birthday"
    t.text "about_me"
    t.datetime "last_login"
    t.string "avatar_file_name"
    t.string "avatar_content_type"
    t.integer "avatar_file_size"
    t.datetime "avatar_updated_at"
    t.string "openid_connect_token"
  end

  add_foreign_key "historical_ratings", "teams"
end
