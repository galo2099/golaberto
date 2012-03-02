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
# It's strongly recommended to check this file into your version control system.

ActiveRecord::Schema.define(:version => 20120302074545) do

  create_table "categories", :force => true do |t|
    t.string "name"
  end

  create_table "championships", :force => true do |t|
    t.string  "name",         :default => "",    :null => false
    t.date    "begin",                           :null => false
    t.date    "end",                             :null => false
    t.integer "point_win",    :default => 3,     :null => false
    t.integer "point_draw",   :default => 1,     :null => false
    t.integer "point_loss",   :default => 0,     :null => false
    t.integer "category_id",  :default => 0,     :null => false
    t.boolean "show_country", :default => false, :null => false
  end

  create_table "comments", :force => true do |t|
    t.string   "title",            :limit => 50, :default => ""
    t.text     "comment"
    t.datetime "created_at",                                     :null => false
    t.integer  "commentable_id",                 :default => 0,  :null => false
    t.string   "commentable_type", :limit => 15, :default => "", :null => false
    t.integer  "user_id",                        :default => 0,  :null => false
  end

  add_index "comments", ["user_id"], :name => "fk_comments_user"

  create_table "delayed_jobs", :force => true do |t|
    t.integer  "priority",   :default => 0
    t.integer  "attempts",   :default => 0
    t.text     "handler"
    t.string   "last_error"
    t.datetime "run_at"
    t.datetime "locked_at"
    t.datetime "failed_at"
    t.string   "locked_by"
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string   "queue"
  end

  create_table "game_goals_versions", :id => false, :force => true do |t|
    t.integer "game_version_id", :default => 0, :null => false
    t.integer "goal_id",         :default => 0, :null => false
  end

  create_table "game_versions", :force => true do |t|
    t.integer  "game_id"
    t.integer  "version"
    t.integer  "home_id",    :default => 0
    t.integer  "away_id",    :default => 0
    t.integer  "phase_id"
    t.integer  "round"
    t.integer  "attendance"
    t.integer  "stadium_id"
    t.integer  "referee_id"
    t.integer  "home_score", :default => 0
    t.integer  "away_score", :default => 0
    t.integer  "home_pen"
    t.integer  "away_pen"
    t.boolean  "played",     :default => false
    t.date     "date"
    t.time     "time"
    t.integer  "updater_id", :default => 0,     :null => false
    t.datetime "updated_at",                    :null => false
    t.integer  "home_aet"
    t.integer  "away_aet"
  end

  create_table "games", :force => true do |t|
    t.integer  "home_id",    :default => 0,     :null => false
    t.integer  "away_id",    :default => 0,     :null => false
    t.integer  "phase_id"
    t.integer  "round"
    t.integer  "attendance"
    t.integer  "stadium_id"
    t.integer  "referee_id"
    t.integer  "home_score", :default => 0,     :null => false
    t.integer  "away_score", :default => 0,     :null => false
    t.integer  "home_pen"
    t.integer  "away_pen"
    t.boolean  "played",     :default => false, :null => false
    t.date     "date",                          :null => false
    t.time     "time"
    t.integer  "version"
    t.integer  "updater_id", :default => 0,     :null => false
    t.datetime "updated_at",                    :null => false
    t.integer  "home_aet"
    t.integer  "away_aet"
  end

  add_index "games", ["away_id"], :name => "index_games_on_away_id"
  add_index "games", ["home_id"], :name => "index_games_on_home_id"

  create_table "goals", :force => true do |t|
    t.integer "player_id", :default => 0,     :null => false
    t.integer "game_id",   :default => 0
    t.integer "team_id",   :default => 0,     :null => false
    t.integer "time",      :default => 0,     :null => false
    t.boolean "penalty",   :default => false, :null => false
    t.boolean "own_goal",  :default => false, :null => false
    t.boolean "aet",       :default => false, :null => false
  end

  add_index "goals", ["player_id", "game_id"], :name => "player"

  create_table "groups", :force => true do |t|
    t.integer "phase_id",      :default => 0,  :null => false
    t.string  "name",          :default => "", :null => false
    t.integer "promoted",      :default => 0,  :null => false
    t.integer "relegated",     :default => 0,  :null => false
    t.integer "odds_progress"
  end

  create_table "open_id_authentication_associations", :force => true do |t|
    t.integer "issued"
    t.integer "lifetime"
    t.string  "handle"
    t.string  "assoc_type"
    t.binary  "server_url"
    t.binary  "secret"
  end

  create_table "open_id_authentication_nonces", :force => true do |t|
    t.integer "timestamp",  :null => false
    t.string  "server_url"
    t.string  "salt",       :null => false
  end

  create_table "phases", :force => true do |t|
    t.integer "championship_id",        :default => 0,                                 :null => false
    t.string  "name",                   :default => "",                                :null => false
    t.integer "order_by",               :default => 0,                                 :null => false
    t.string  "sort",                   :default => "pt, w, gd, gf, gp, g_away, name", :null => false
    t.integer "bonus_points",           :default => 0,                                 :null => false
    t.integer "bonus_points_threshold", :default => 0,                                 :null => false
  end

  add_index "phases", ["championship_id"], :name => "championship"

  create_table "player_games", :force => true do |t|
    t.integer "player_id", :default => 0,     :null => false
    t.integer "game_id",   :default => 0,     :null => false
    t.integer "team_id",   :default => 0,     :null => false
    t.integer "on",        :default => 0,     :null => false
    t.integer "off",       :default => 0,     :null => false
    t.boolean "yellow",    :default => false, :null => false
    t.boolean "red",       :default => false, :null => false
  end

  create_table "players", :force => true do |t|
    t.string "name",                   :default => "", :null => false
    t.string "position",  :limit => 3
    t.date   "birth"
    t.string "country"
    t.string "full_name"
  end

  create_table "referee_champs", :force => true do |t|
    t.integer "referee_id",      :default => 0, :null => false
    t.integer "championship_id", :default => 0, :null => false
  end

  create_table "referees", :force => true do |t|
    t.string "name",     :default => "", :null => false
    t.string "location"
  end

  create_table "roles", :force => true do |t|
    t.string "name"
  end

  create_table "roles_users", :id => false, :force => true do |t|
    t.integer "role_id"
    t.integer "user_id"
  end

  add_index "roles_users", ["role_id"], :name => "index_roles_users_on_role_id"
  add_index "roles_users", ["user_id"], :name => "index_roles_users_on_user_id"

  create_table "stadia", :force => true do |t|
    t.string "name",      :default => "", :null => false
    t.string "full_name"
    t.string "city"
    t.string "country"
  end

  create_table "team_groups", :force => true do |t|
    t.integer "group_id",       :default => 0, :null => false
    t.integer "team_id",        :default => 0, :null => false
    t.integer "add_sub",        :default => 0, :null => false
    t.integer "bias",           :default => 0, :null => false
    t.text    "comment"
    t.float   "first_odds"
    t.float   "promoted_odds"
    t.float   "relegated_odds"
  end

  add_index "team_groups", ["group_id", "team_id"], :name => "group", :unique => true
  add_index "team_groups", ["id"], :name => "id"

  create_table "team_players", :force => true do |t|
    t.integer "team_id",         :default => 0, :null => false
    t.integer "player_id",       :default => 0, :null => false
    t.integer "championship_id", :default => 0, :null => false
  end

  create_table "teams", :force => true do |t|
    t.string  "name",       :default => "", :null => false
    t.string  "country",    :default => "", :null => false
    t.string  "logo"
    t.string  "city"
    t.integer "stadium_id"
    t.date    "foundation"
    t.string  "full_name"
  end

  create_table "users", :force => true do |t|
    t.string   "login"
    t.string   "email"
    t.string   "crypted_password",          :limit => 40
    t.string   "salt",                      :limit => 40
    t.datetime "created_at"
    t.datetime "updated_at"
    t.string   "remember_token"
    t.datetime "remember_token_expires_at"
    t.string   "identity_url"
    t.string   "name",                      :limit => 30
    t.string   "location",                  :limit => 100
    t.date     "birthday"
    t.text     "about_me"
    t.datetime "last_login"
  end

end
