# This file is auto-generated from the current state of the database. Instead of editing this file, 
# please use the migrations feature of Active Record to incrementally modify your database, and
# then regenerate this schema definition.
#
# Note that this schema.rb definition is the authoritative source for your database schema. If you need
# to create the application database on another system, you should be using db:schema:load, not running
# all the migrations from scratch. The latter is a flawed and unsustainable approach (the more migrations
# you'll amass, the slower it'll run and the greater likelihood for issues).
#
# It's strongly recommended to check this file into your version control system.

ActiveRecord::Schema.define(:version => 20081125013901) do

  create_table "categories", :force => true do |t|
    t.string "name"
  end

  create_table "championships", :force => true do |t|
    t.string  "name",                      :default => "", :null => false
    t.date    "begin",                                     :null => false
    t.date    "end",                                       :null => false
    t.integer "point_win",   :limit => 4,  :default => 3,  :null => false
    t.integer "point_draw",  :limit => 4,  :default => 1,  :null => false
    t.integer "point_loss",  :limit => 4,  :default => 0,  :null => false
    t.integer "category_id", :limit => 20, :default => 0,  :null => false
  end

  create_table "comments", :force => true do |t|
    t.string   "title",            :limit => 50, :default => ""
    t.text     "comment"
    t.datetime "created_at",                                     :null => false
    t.integer  "commentable_id",   :limit => 11, :default => 0,  :null => false
    t.string   "commentable_type", :limit => 15, :default => "", :null => false
    t.integer  "user_id",          :limit => 11, :default => 0,  :null => false
  end

  add_index "comments", ["user_id"], :name => "fk_comments_user"

  create_table "delayed_jobs", :force => true do |t|
    t.integer  "priority",   :limit => 11, :default => 0
    t.integer  "attempts",   :limit => 11, :default => 0
    t.text     "handler"
    t.string   "last_error"
    t.datetime "run_at"
    t.datetime "locked_at"
    t.datetime "failed_at"
    t.string   "locked_by"
    t.datetime "created_at"
    t.datetime "updated_at"
  end

  create_table "game_goals_versions", :id => false, :force => true do |t|
    t.integer "game_version_id", :limit => 11, :default => 0, :null => false
    t.integer "goal_id",         :limit => 11, :default => 0, :null => false
  end

  create_table "game_versions", :force => true do |t|
    t.integer  "game_id",    :limit => 11
    t.integer  "version",    :limit => 11
    t.integer  "home_id",    :limit => 20, :default => 0
    t.integer  "away_id",    :limit => 20, :default => 0
    t.integer  "phase_id",   :limit => 20
    t.integer  "round",      :limit => 4
    t.integer  "attendance", :limit => 9
    t.integer  "stadium_id", :limit => 20
    t.integer  "referee_id", :limit => 20
    t.integer  "home_score", :limit => 2,  :default => 0
    t.integer  "away_score", :limit => 2,  :default => 0
    t.integer  "home_pen",   :limit => 2
    t.integer  "away_pen",   :limit => 2
    t.boolean  "played",                   :default => false
    t.date     "date"
    t.time     "time"
    t.integer  "updater_id", :limit => 20, :default => 0,     :null => false
    t.datetime "updated_at",                                  :null => false
  end

  create_table "games", :force => true do |t|
    t.integer  "home_id",    :limit => 20, :default => 0,     :null => false
    t.integer  "away_id",    :limit => 20, :default => 0,     :null => false
    t.integer  "phase_id",   :limit => 20
    t.integer  "round",      :limit => 4
    t.integer  "attendance", :limit => 9
    t.integer  "stadium_id", :limit => 20
    t.integer  "referee_id", :limit => 20
    t.integer  "home_score", :limit => 2,  :default => 0,     :null => false
    t.integer  "away_score", :limit => 2,  :default => 0,     :null => false
    t.integer  "home_pen",   :limit => 2
    t.integer  "away_pen",   :limit => 2
    t.boolean  "played",                   :default => false, :null => false
    t.date     "date",                                        :null => false
    t.time     "time"
    t.integer  "version",    :limit => 11
    t.integer  "updater_id", :limit => 20, :default => 0,     :null => false
    t.datetime "updated_at",                                  :null => false
  end

  create_table "goals", :force => true do |t|
    t.integer "player_id", :limit => 20, :default => 0,     :null => false
    t.integer "game_id",   :limit => 20, :default => 0
    t.integer "team_id",   :limit => 20, :default => 0,     :null => false
    t.integer "time",      :limit => 4,  :default => 0,     :null => false
    t.boolean "penalty",                 :default => false, :null => false
    t.boolean "own_goal",                :default => false, :null => false
  end

  add_index "goals", ["player_id", "game_id"], :name => "player"

  create_table "groups", :force => true do |t|
    t.integer "phase_id",      :limit => 20, :default => 0,  :null => false
    t.string  "name",                        :default => "", :null => false
    t.integer "promoted",      :limit => 2,  :default => 0,  :null => false
    t.integer "relegated",     :limit => 2,  :default => 0,  :null => false
    t.integer "odds_progress", :limit => 11
  end

  create_table "phases", :force => true do |t|
    t.integer "championship_id",        :limit => 20, :default => 0,                                 :null => false
    t.string  "name",                                 :default => "",                                :null => false
    t.integer "order_by",               :limit => 4,  :default => 0,                                 :null => false
    t.string  "sort",                                 :default => "pt, w, gd, gf, gp, g_away, name", :null => false
    t.integer "bonus_points",           :limit => 4,  :default => 0,                                 :null => false
    t.integer "bonus_points_threshold", :limit => 4,  :default => 0,                                 :null => false
  end

  add_index "phases", ["championship_id"], :name => "championship"

  create_table "player_games", :force => true do |t|
    t.integer "player_id", :limit => 20, :default => 0,     :null => false
    t.integer "game_id",   :limit => 20, :default => 0,     :null => false
    t.integer "team_id",   :limit => 20, :default => 0,     :null => false
    t.integer "on",        :limit => 20, :default => 0,     :null => false
    t.integer "off",       :limit => 20, :default => 0,     :null => false
    t.boolean "yellow",                  :default => false, :null => false
    t.boolean "red",                     :default => false, :null => false
  end

  create_table "players", :force => true do |t|
    t.string "name",                   :default => "", :null => false
    t.string "position",  :limit => 3
    t.date   "birth"
    t.string "country"
    t.string "full_name"
  end

  create_table "plugin_schema_info", :id => false, :force => true do |t|
    t.string  "plugin_name"
    t.integer "version",     :limit => 11
  end

  create_table "referee_champs", :force => true do |t|
    t.integer "referee_id",      :limit => 20, :default => 0, :null => false
    t.integer "championship_id", :limit => 20, :default => 0, :null => false
  end

  create_table "referees", :force => true do |t|
    t.string "name",     :default => "", :null => false
    t.string "location"
  end

  create_table "schema_info", :id => false, :force => true do |t|
    t.integer "version", :limit => 11
  end

  create_table "stadia", :force => true do |t|
    t.string "name",      :default => "", :null => false
    t.string "full_name"
    t.string "city"
    t.string "country"
  end

  create_table "team_groups", :force => true do |t|
    t.integer "group_id",       :limit => 20, :default => 0, :null => false
    t.integer "team_id",        :limit => 20, :default => 0, :null => false
    t.integer "add_sub",        :limit => 4,  :default => 0, :null => false
    t.integer "bias",           :limit => 4,  :default => 0, :null => false
    t.text    "comment"
    t.float   "first_odds"
    t.float   "promoted_odds"
    t.float   "relegated_odds"
  end

  add_index "team_groups", ["group_id", "team_id"], :name => "group", :unique => true
  add_index "team_groups", ["id"], :name => "id"

  create_table "team_players", :force => true do |t|
    t.integer "team_id",         :limit => 20, :default => 0, :null => false
    t.integer "player_id",       :limit => 20, :default => 0, :null => false
    t.integer "championship_id", :limit => 20, :default => 0, :null => false
  end

  create_table "teams", :force => true do |t|
    t.string  "name",                     :default => "", :null => false
    t.string  "country",                  :default => "", :null => false
    t.string  "logo"
    t.string  "city"
    t.integer "stadium_id", :limit => 20
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
  end

end
