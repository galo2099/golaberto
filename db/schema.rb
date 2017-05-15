# encoding: UTF-8
# This file is auto-generated from the current state of the database. Instead
# of editing this file, please use the migrations feature of Active Record to
# incrementally modify your database, and then regenerate this schema definition.
#
# Note that this schema.rb definition is the authoritative source for your
# database schema. If you need to create the application database on another
# system, you should be using db:schema:load, not running all the migrations
# from scratch. The latter is a flawed and unsustainable approach (the more migrations
# you'll amass, the slower it'll run and the greater likelihood for issues).
#
# It's strongly recommended that you check this file into your version control system.

ActiveRecord::Schema.define(version: 20170515041130) do

  create_table "categories", force: :cascade do |t|
    t.string "name", limit: 255
  end

  create_table "championships", force: :cascade do |t|
    t.string   "name",         limit: 255, default: "",    null: false
    t.date     "begin",                                    null: false
    t.date     "end",                                      null: false
    t.integer  "point_win",    limit: 4,   default: 3,     null: false
    t.integer  "point_draw",   limit: 4,   default: 1,     null: false
    t.integer  "point_loss",   limit: 4,   default: 0,     null: false
    t.integer  "category_id",  limit: 4,   default: 0,     null: false
    t.boolean  "show_country",             default: false, null: false
    t.datetime "created_at",                               null: false
    t.datetime "updated_at",                               null: false
  end

  create_table "collation_test", id: false, force: :cascade do |t|
    t.string "name", limit: 255, null: false
  end

  create_table "comments", force: :cascade do |t|
    t.string   "title",            limit: 50,    default: ""
    t.text     "comment",          limit: 65535
    t.datetime "created_at",                                  null: false
    t.integer  "commentable_id",   limit: 4,     default: 0,  null: false
    t.string   "commentable_type", limit: 15,    default: "", null: false
    t.integer  "user_id",          limit: 4,     default: 0,  null: false
  end

  add_index "comments", ["user_id"], name: "fk_comments_user", using: :btree

  create_table "delayed_jobs", force: :cascade do |t|
    t.integer  "priority",   limit: 4,     default: 0
    t.integer  "attempts",   limit: 4,     default: 0
    t.text     "handler",    limit: 65535
    t.string   "last_error", limit: 255
    t.datetime "run_at"
    t.datetime "locked_at"
    t.datetime "failed_at"
    t.string   "locked_by",  limit: 255
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string   "queue",      limit: 255
  end

  create_table "game_goals_versions", id: false, force: :cascade do |t|
    t.integer "game_version_id", limit: 4, default: 0, null: false
    t.integer "goal_id",         limit: 4, default: 0, null: false
  end

  create_table "game_versions", force: :cascade do |t|
    t.integer  "game_id",    limit: 4
    t.integer  "version",    limit: 4
    t.integer  "home_id",    limit: 4, default: 0
    t.integer  "away_id",    limit: 4, default: 0
    t.integer  "phase_id",   limit: 4
    t.integer  "round",      limit: 4
    t.integer  "attendance", limit: 4
    t.integer  "stadium_id", limit: 4
    t.integer  "referee_id", limit: 4
    t.integer  "home_score", limit: 4, default: 0
    t.integer  "away_score", limit: 4, default: 0
    t.integer  "home_pen",   limit: 4
    t.integer  "away_pen",   limit: 4
    t.boolean  "played",               default: false
    t.integer  "updater_id", limit: 4, default: 0,     null: false
    t.datetime "updated_at",                           null: false
    t.integer  "home_aet",   limit: 4
    t.integer  "away_aet",   limit: 4
    t.datetime "date",                                 null: false
    t.boolean  "has_time",             default: false
    t.integer  "home_field", limit: 4, default: 0,     null: false
  end

  add_index "game_versions", ["game_id"], name: "index_game_versions_on_game_id", using: :btree
  add_index "game_versions", ["updater_id"], name: "index_game_versions_on_updater_id", using: :btree

  create_table "games", force: :cascade do |t|
    t.integer  "home_id",    limit: 4, default: 0,     null: false
    t.integer  "away_id",    limit: 4, default: 0,     null: false
    t.integer  "phase_id",   limit: 4
    t.integer  "round",      limit: 4
    t.integer  "attendance", limit: 4
    t.integer  "stadium_id", limit: 4
    t.integer  "referee_id", limit: 4
    t.integer  "home_score", limit: 4, default: 0,     null: false
    t.integer  "away_score", limit: 4, default: 0,     null: false
    t.integer  "home_pen",   limit: 4
    t.integer  "away_pen",   limit: 4
    t.boolean  "played",               default: false, null: false
    t.integer  "version",    limit: 4
    t.integer  "updater_id", limit: 4, default: 0,     null: false
    t.datetime "updated_at",                           null: false
    t.integer  "home_aet",   limit: 4
    t.integer  "away_aet",   limit: 4
    t.datetime "date",                                 null: false
    t.boolean  "has_time",             default: false
    t.integer  "home_field", limit: 4, default: 0,     null: false
  end

  add_index "games", ["away_id"], name: "index_games_on_away_id", using: :btree
  add_index "games", ["home_id"], name: "index_games_on_home_id", using: :btree
  add_index "games", ["phase_id"], name: "index_games_on_phase_id", using: :btree
  add_index "games", ["played"], name: "index_games_on_played", using: :btree

  create_table "goals", force: :cascade do |t|
    t.integer  "player_id",  limit: 4, default: 0,     null: false
    t.integer  "game_id",    limit: 4, default: 0
    t.integer  "team_id",    limit: 4, default: 0,     null: false
    t.integer  "time",       limit: 4, default: 0,     null: false
    t.boolean  "penalty",              default: false, null: false
    t.boolean  "own_goal",             default: false, null: false
    t.boolean  "aet",                  default: false, null: false
    t.datetime "created_at",                           null: false
    t.datetime "updated_at",                           null: false
  end

  add_index "goals", ["player_id", "game_id"], name: "player", using: :btree

  create_table "groups", force: :cascade do |t|
    t.integer  "phase_id",      limit: 4,   default: 0,  null: false
    t.string   "name",          limit: 255, default: "", null: false
    t.integer  "promoted",      limit: 4,   default: 0,  null: false
    t.integer  "relegated",     limit: 4,   default: 0,  null: false
    t.integer  "odds_progress", limit: 4
    t.datetime "created_at",                             null: false
    t.datetime "updated_at",                             null: false
  end

  create_table "open_id_authentication_associations", force: :cascade do |t|
    t.integer "issued",     limit: 4
    t.integer "lifetime",   limit: 4
    t.string  "handle",     limit: 255
    t.string  "assoc_type", limit: 255
    t.binary  "server_url", limit: 65535
    t.binary  "secret",     limit: 65535
  end

  create_table "open_id_authentication_nonces", force: :cascade do |t|
    t.integer "timestamp",  limit: 4,   null: false
    t.string  "server_url", limit: 255
    t.string  "salt",       limit: 255, null: false
  end

  create_table "phases", force: :cascade do |t|
    t.integer  "championship_id",        limit: 4,   default: 0,                                 null: false
    t.string   "name",                   limit: 255, default: "",                                null: false
    t.integer  "order_by",               limit: 4,   default: 0,                                 null: false
    t.string   "sort",                   limit: 255, default: "pt, w, gd, gf, gp, g_away, name", null: false
    t.integer  "bonus_points",           limit: 4,   default: 0,                                 null: false
    t.integer  "bonus_points_threshold", limit: 4,   default: 0,                                 null: false
    t.datetime "created_at",                                                                     null: false
    t.datetime "updated_at",                                                                     null: false
  end

  add_index "phases", ["championship_id"], name: "championship", using: :btree

  create_table "player_games", force: :cascade do |t|
    t.integer "player_id", limit: 4, default: 0,     null: false
    t.integer "game_id",   limit: 4, default: 0,     null: false
    t.integer "team_id",   limit: 4, default: 0,     null: false
    t.integer "on",        limit: 4, default: 0,     null: false
    t.integer "off",       limit: 4, default: 0,     null: false
    t.boolean "yellow",              default: false, null: false
    t.boolean "red",                 default: false, null: false
  end

  create_table "players", force: :cascade do |t|
    t.string   "name",       limit: 255, default: "", null: false
    t.string   "position",   limit: 3
    t.date     "birth"
    t.string   "country",    limit: 255
    t.string   "full_name",  limit: 255
    t.datetime "created_at",                          null: false
    t.datetime "updated_at",                          null: false
  end

  create_table "referee_champs", force: :cascade do |t|
    t.integer "referee_id",      limit: 4, default: 0, null: false
    t.integer "championship_id", limit: 4, default: 0, null: false
  end

  create_table "referees", force: :cascade do |t|
    t.string   "name",       limit: 255, default: "", null: false
    t.string   "location",   limit: 255
    t.datetime "created_at",                          null: false
    t.datetime "updated_at",                          null: false
  end

  create_table "roles", force: :cascade do |t|
    t.string "name", limit: 255
  end

  create_table "roles_users", id: false, force: :cascade do |t|
    t.integer "role_id", limit: 4
    t.integer "user_id", limit: 4
  end

  add_index "roles_users", ["role_id"], name: "index_roles_users_on_role_id", using: :btree
  add_index "roles_users", ["user_id"], name: "index_roles_users_on_user_id", using: :btree

  create_table "stadia", force: :cascade do |t|
    t.string   "name",       limit: 255, default: "", null: false
    t.string   "full_name",  limit: 255
    t.string   "city",       limit: 255
    t.string   "country",    limit: 255
    t.datetime "created_at",                          null: false
    t.datetime "updated_at",                          null: false
  end

  create_table "team_groups", force: :cascade do |t|
    t.integer  "group_id",   limit: 4,     default: 0, null: false
    t.integer  "team_id",    limit: 4,     default: 0, null: false
    t.integer  "add_sub",    limit: 4,     default: 0, null: false
    t.integer  "bias",       limit: 4,     default: 0, null: false
    t.text     "comment",    limit: 65535
    t.datetime "created_at",                           null: false
    t.datetime "updated_at",                           null: false
    t.text     "odds",       limit: 65535
  end

  add_index "team_groups", ["group_id", "team_id"], name: "group", unique: true, using: :btree
  add_index "team_groups", ["id"], name: "id", using: :btree

  create_table "team_players", force: :cascade do |t|
    t.integer  "team_id",         limit: 4, default: 0, null: false
    t.integer  "player_id",       limit: 4, default: 0, null: false
    t.integer  "championship_id", limit: 4, default: 0, null: false
    t.datetime "created_at",                            null: false
    t.datetime "updated_at",                            null: false
  end

  create_table "teams", force: :cascade do |t|
    t.string   "name",              limit: 255, default: "", null: false
    t.string   "country",           limit: 255, default: "", null: false
    t.string   "legacy_logo",       limit: 255
    t.string   "city",              limit: 255
    t.integer  "stadium_id",        limit: 4
    t.date     "foundation"
    t.string   "full_name",         limit: 255
    t.string   "logo_file_name",    limit: 255
    t.string   "logo_content_type", limit: 255
    t.integer  "logo_file_size",    limit: 4
    t.datetime "logo_updated_at"
    t.datetime "created_at",                                 null: false
    t.datetime "updated_at",                                 null: false
    t.float    "rating",            limit: 24
    t.float    "off_rating",        limit: 24
    t.float    "def_rating",        limit: 24
  end

  create_table "users", force: :cascade do |t|
    t.string   "login",                     limit: 255
    t.string   "email",                     limit: 255
    t.string   "crypted_password",          limit: 40
    t.string   "salt",                      limit: 40
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string   "remember_token",            limit: 255
    t.datetime "remember_token_expires_at"
    t.string   "identity_url",              limit: 255
    t.string   "name",                      limit: 30
    t.string   "location",                  limit: 100
    t.date     "birthday"
    t.text     "about_me",                  limit: 65535
    t.datetime "last_login"
    t.string   "avatar_file_name",          limit: 255
    t.string   "avatar_content_type",       limit: 255
    t.integer  "avatar_file_size",          limit: 4
    t.datetime "avatar_updated_at"
    t.string   "openid_connect_token",      limit: 255
  end

end
