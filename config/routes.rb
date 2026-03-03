Rails.application.routes.draw do
  # For details on the DSL available within this file, see http://guides.rubyonrails.org/routing.html
  # The priority is based upon order of creation:
  # first created -> highest priority.

  # Sample of regular route:
  #   match 'products/:id' => 'catalog#view'
  # Keep in mind you can assign values other than :controller and :action

  # Sample of named route:
  #   match 'products/:id/purchase' => 'catalog#purchase', :as => :purchase
  # This route can be invoked with purchase_url(:id => product.id)

  # Sample resource route (maps HTTP verbs to controller actions automatically):
  #   resources :products

  # Sample resource route with options:
  #   resources :products do
  #     member do
  #       get 'short'
  #       post 'toggle'
  #     end
  #
  #     collection do
  #       get 'sold'
  #     end
  #   end

  # Sample resource route with sub-resources:
  #   resources :products do
  #     resources :comments, :sales
  #     resource :seller
  #   end

  # Sample resource route with more complex sub-resources
  #   resources :products do
  #     resources :comments
  #     resources :sales do
  #       get 'recent', :on => :collection
  #     end
  #   end

  # Sample resource route within a namespace:
  #   namespace :admin do
  #     # Directs /admin/products/* to Admin::ProductsController
  #     # (app/controllers/admin/products_controller.rb)
  #     resources :products
  #   end

  # You can have the root of your site routed with "root"
  # just remember to delete public/index.html.
  # root :to => 'welcome#index'

  # See how all your routes lay out with "rake routes"

  # This is a legacy wild controller route that's not recommended for RESTful applications.
  # Note: This route will make all actions in every controller accessible via GET requests.
  # match ':controller(/:action(/:id))(.:format)'

  # You can have the root of your site routed by hooking up '' 
  # -- just remember to delete public/index.html.
  root :to => 'home#index'

  # Legacy route to redirect old links to /en_US paths to the english route.
  get '/en_US/*other', to: redirect(host: APP_CONFIG["en_US_host"], path: '/%{other}')

  get '/auth/google/callback', to: "account#google_signin"
  post '/auth/google/onetap_callback', to: "account#google_onetap"
  get '/auth/failure', to: 'account#failure'
  get 'map/static', to: 'map#static', as: :map_static

  get 'groups/team_list.js' => 'group#team_list'

  # map championship actions
  match 'championship/show/:id/phases/:phase' => 'championship#phases', via: :get
  match 'championship/show/:id/phases/:phase/team_json/' => 'championship#team_json', via: :get
  match 'championship/show/:id/games/:phase(/group/:group)(/round/:round)(/p/:page)' => 'championship#games', :constraints => { :page => /\d+/ }, :defaults => { :page => 1 }, via: :get
  match 'championship/show/:id/new_game/:phase' => 'championship#new_game', via: :get
  match 'championship/show/:id/team/:team' => 'championship#team', via: :get
  match 'championship/show/:id/team/:team/player/:player' => 'championship#player_show', via: :get
  match 'championship/show/:id/players/list' => 'championship#player_list', via: :get

  match 'game/list(/:type)(/cat/:category)(/p/:page)' => 'game#list', :constraints => { :page => /\d+/ }, :defaults => { :type => :scheduled, :page => 1, :category => 1 }, via: :get
  match 'game/list/played(/:type)(/cat/:category)(/p/:page)' => 'game#list', :constraints => { :page => /\d+/ }, :defaults => { :type => :played, :page => 1, :category => 1 }, via: :get

  match 'team/games/:type/:id(/cat/:category)(/p/:page)' => 'team#games', :constraints => { :page => /\d+/ }, :defaults => { :category => 1, :page => 1 }, via: :get
  match 'team/list/:team_type(/p/:page)' => 'team#list', :constraints => { :page => /\d+/ }, :defaults => { :team_type => "club", :page => 1 }, via: :get

  match 'player/games/:type/:id(/cat/:category)(/p/:page)' => 'player#games', :constraints => { :page => /\d+/ }, :defaults => { :category => 1, :page => 1 }, via: :get

  get 'championship/top_goalscorers/:id', to: redirect { |_params, _request|
    Rails.application.routes.url_helpers.url_for host: _request.host, port: _request.port, controller: :championship, action: :player_list, id: _params[:id]
  }

  get 'championship/players/:id', to: redirect {  |_params, _request|
    Rails.application.routes.url_helpers.url_for host: _request.host, port: _request.port, controller: :championship, action: :player_list, id: _params[:id]
  }

  # Install explicit legacy routes as the lowest priority.
  # This preserves old /controller/action(/id) links without relying on
  # deprecated dynamic :controller/:action route segments.
  #
  # The list below is intentionally explicit (not auto-generated) and limited
  # to legacy controller/actions with matching view templates plus required
  # inline-rendered endpoints still used by the UI.
  legacy_verbs = [:get, :post, :patch]
  match 'account/failure(/:id)(.:format)', to: 'account#failure', via: legacy_verbs, as: nil
  match 'account/google_onetap(/:id)(.:format)', to: 'account#google_onetap', via: legacy_verbs, as: nil
  match 'account/google_signin(/:id)(.:format)', to: 'account#google_signin', via: legacy_verbs, as: nil
  match 'account/login(/:id)(.:format)', to: 'account#login', via: legacy_verbs, as: nil
  match 'account/logout(/:id)(.:format)', to: 'account#logout', via: legacy_verbs, as: nil
  match 'account/signup(/:id)(.:format)', to: 'account#signup', via: legacy_verbs, as: nil

  match 'championship/create(/:id)(.:format)', to: 'championship#create', via: legacy_verbs, as: nil
  match 'championship/crowd(/:id)(.:format)', to: 'championship#crowd', via: legacy_verbs, as: nil
  match 'championship/destroy(/:id)(.:format)', to: 'championship#destroy', via: legacy_verbs, as: nil
  match 'championship/edit(/:id)(.:format)', to: 'championship#edit', via: legacy_verbs, as: nil
  match 'championship/games(/:id)(.:format)', to: 'championship#games', via: legacy_verbs, as: nil
  match 'championship/index(/:id)(.:format)', to: 'championship#index', via: legacy_verbs, as: nil
  match 'championship/list(/:id)(.:format)', to: 'championship#list', via: legacy_verbs, as: nil
  match 'championship/new(/:id)(.:format)', to: 'championship#new', via: legacy_verbs, as: nil
  match 'championship/new_game(/:id)(.:format)', to: 'championship#new_game', via: legacy_verbs, as: nil
  match 'championship/phases(/:id)(.:format)', to: 'championship#phases', via: legacy_verbs, as: nil
  match 'championship/player_list(/:id)(.:format)', to: 'championship#player_list', via: legacy_verbs, as: nil
  match 'championship/player_show(/:id)(.:format)', to: 'championship#player_show', via: legacy_verbs, as: nil
  match 'championship/show(/:id)(.:format)', to: 'championship#show', via: legacy_verbs, as: nil
  match 'championship/spi_eval(/:id)(.:format)', to: 'championship#spi_eval', via: legacy_verbs, as: nil
  match 'championship/team(/:id)(.:format)', to: 'championship#team', via: legacy_verbs, as: nil
  match 'championship/team_json(/:id)(.:format)', to: 'championship#team_json', via: legacy_verbs, as: nil
  match 'championship/update(/:id)(.:format)', to: 'championship#update', via: legacy_verbs, as: nil

  match 'comment/destroy(/:id)(.:format)', to: 'comment#destroy', via: legacy_verbs, as: nil
  match 'comment/new(/:id)(.:format)', to: 'comment#new', via: legacy_verbs, as: nil

  match 'game/create(/:id)(.:format)', to: 'game#create', via: legacy_verbs, as: nil
  match 'game/create_referee_for_edit(/:id)(.:format)', to: 'game#create_referee_for_edit', via: legacy_verbs, as: nil
  match 'game/create_stadium_for_edit(/:id)(.:format)', to: 'game#create_stadium_for_edit', via: legacy_verbs, as: nil
  match 'game/destroy(/:id)(.:format)', to: 'game#destroy', via: legacy_verbs, as: nil
  match 'game/edit(/:id)(.:format)', to: 'game#edit', via: legacy_verbs, as: nil
  match 'game/edit_squad(/:id)(.:format)', to: 'game#edit_squad', via: legacy_verbs, as: nil
  match 'game/index(/:id)(.:format)', to: 'game#index', via: legacy_verbs, as: nil
  match 'game/insert_team_player(/:id)(.:format)', to: 'game#insert_team_player', via: legacy_verbs, as: nil
  match 'game/list(/:id)(.:format)', to: 'game#list', via: legacy_verbs, as: nil
  match 'game/list_players(/:id)(.:format)', to: 'game#list_players', via: legacy_verbs, as: nil
  match 'game/show(/:id)(.:format)', to: 'game#show', via: legacy_verbs, as: nil
  match 'game/update(/:id)(.:format)', to: 'game#update', via: legacy_verbs, as: nil
  match 'game/update_squad(/:id)(.:format)', to: 'game#update_squad', via: legacy_verbs, as: nil

  match 'group/destroy(/:id)(.:format)', to: 'group#destroy', via: legacy_verbs, as: nil
  match 'group/edit(/:id)(.:format)', to: 'group#edit', via: legacy_verbs, as: nil
  match 'group/odds_progress(/:id)(.:format)', to: 'group#odds_progress', via: legacy_verbs, as: nil
  match 'group/start_odds_history_backfill(/:id)(.:format)', to: 'group#start_odds_history_backfill', via: legacy_verbs, as: nil
  match 'group/team_list(/:id)(.:format)', to: 'group#team_list', via: legacy_verbs, as: nil
  match 'group/update(/:id)(.:format)', to: 'group#update', via: legacy_verbs, as: nil
  match 'group/update_odds(/:id)(.:format)', to: 'group#update_odds', via: legacy_verbs, as: nil

  match 'phase/add_groups(/:id)(.:format)', to: 'phase#add_groups', via: legacy_verbs, as: nil
  match 'phase/create(/:id)(.:format)', to: 'phase#create', via: legacy_verbs, as: nil
  match 'phase/destroy(/:id)(.:format)', to: 'phase#destroy', via: legacy_verbs, as: nil
  match 'phase/edit(/:id)(.:format)', to: 'phase#edit', via: legacy_verbs, as: nil
  match 'phase/new(/:id)(.:format)', to: 'phase#new', via: legacy_verbs, as: nil
  match 'phase/start_scrape(/:id)(.:format)', to: 'phase#start_scrape', via: legacy_verbs, as: nil
  match 'phase/update(/:id)(.:format)', to: 'phase#update', via: legacy_verbs, as: nil

  match 'player/create(/:id)(.:format)', to: 'player#create', via: legacy_verbs, as: nil
  match 'player/destroy(/:id)(.:format)', to: 'player#destroy', via: legacy_verbs, as: nil
  match 'player/destroy_team(/:id)(.:format)', to: 'player#destroy_team', via: legacy_verbs, as: nil
  match 'player/edit(/:id)(.:format)', to: 'player#edit', via: legacy_verbs, as: nil
  match 'player/games(/:id)(.:format)', to: 'player#games', via: legacy_verbs, as: nil
  match 'player/index(/:id)(.:format)', to: 'player#index', via: legacy_verbs, as: nil
  match 'player/list(/:id)(.:format)', to: 'player#list', via: legacy_verbs, as: nil
  match 'player/new(/:id)(.:format)', to: 'player#new', via: legacy_verbs, as: nil
  match 'player/show(/:id)(.:format)', to: 'player#show', via: legacy_verbs, as: nil
  match 'player/update(/:id)(.:format)', to: 'player#update', via: legacy_verbs, as: nil
  match 'player/update_rating(/:id)(.:format)', to: 'player#update_rating', via: legacy_verbs, as: nil

  match 'referee/create(/:id)(.:format)', to: 'referee#create', via: legacy_verbs, as: nil
  match 'referee/destroy(/:id)(.:format)', to: 'referee#destroy', via: legacy_verbs, as: nil
  match 'referee/edit(/:id)(.:format)', to: 'referee#edit', via: legacy_verbs, as: nil
  match 'referee/games(/:id)(.:format)', to: 'referee#games', via: legacy_verbs, as: nil
  match 'referee/index(/:id)(.:format)', to: 'referee#index', via: legacy_verbs, as: nil
  match 'referee/list(/:id)(.:format)', to: 'referee#list', via: legacy_verbs, as: nil
  match 'referee/new(/:id)(.:format)', to: 'referee#new', via: legacy_verbs, as: nil
  match 'referee/show(/:id)(.:format)', to: 'referee#show', via: legacy_verbs, as: nil
  match 'referee/update(/:id)(.:format)', to: 'referee#update', via: legacy_verbs, as: nil

  match 'stadium/create(/:id)(.:format)', to: 'stadium#create', via: legacy_verbs, as: nil
  match 'stadium/destroy(/:id)(.:format)', to: 'stadium#destroy', via: legacy_verbs, as: nil
  match 'stadium/edit(/:id)(.:format)', to: 'stadium#edit', via: legacy_verbs, as: nil
  match 'stadium/games(/:id)(.:format)', to: 'stadium#games', via: legacy_verbs, as: nil
  match 'stadium/index(/:id)(.:format)', to: 'stadium#index', via: legacy_verbs, as: nil
  match 'stadium/list(/:id)(.:format)', to: 'stadium#list', via: legacy_verbs, as: nil
  match 'stadium/new(/:id)(.:format)', to: 'stadium#new', via: legacy_verbs, as: nil
  match 'stadium/show(/:id)(.:format)', to: 'stadium#show', via: legacy_verbs, as: nil
  match 'stadium/update(/:id)(.:format)', to: 'stadium#update', via: legacy_verbs, as: nil

  match 'team/auto_complete_for_team_name(/:id)(.:format)', to: 'team#auto_complete_for_team_name', via: legacy_verbs, as: nil
  match 'team/create(/:id)(.:format)', to: 'team#create', via: legacy_verbs, as: nil
  match 'team/destroy(/:id)(.:format)', to: 'team#destroy', via: legacy_verbs, as: nil
  match 'team/edit(/:id)(.:format)', to: 'team#edit', via: legacy_verbs, as: nil
  match 'team/games(/:id)(.:format)', to: 'team#games', via: legacy_verbs, as: nil
  match 'team/historic_ratings(/:id)(.:format)', to: 'team#historic_ratings', via: legacy_verbs, as: nil
  match 'team/historical_rating(/:id)(.:format)', to: 'team#historical_rating', via: legacy_verbs, as: nil
  match 'team/index(/:id)(.:format)', to: 'team#index', via: legacy_verbs, as: nil
  match 'team/list(/:id)(.:format)', to: 'team#list', via: legacy_verbs, as: nil
  match 'team/new(/:id)(.:format)', to: 'team#new', via: legacy_verbs, as: nil
  match 'team/show(/:id)(.:format)', to: 'team#show', via: legacy_verbs, as: nil
  match 'team/update(/:id)(.:format)', to: 'team#update', via: legacy_verbs, as: nil
  match 'team/update_rating(/:id)(.:format)', to: 'team#update_rating', via: legacy_verbs, as: nil

  match 'team_group/index(/:id)(.:format)', to: 'team_group#index', via: legacy_verbs, as: nil
  match 'team_group/list(/:id)(.:format)', to: 'team_group#list', via: legacy_verbs, as: nil

  match 'user/edit(/:id)(.:format)', to: 'user#edit', via: legacy_verbs, as: nil
  match 'user/list(/:id)(.:format)', to: 'user#list', via: legacy_verbs, as: nil
  match 'user/list_edits(/:id)(.:format)', to: 'user#list_edits', via: legacy_verbs, as: nil
  match 'user/show(/:id)(.:format)', to: 'user#show', via: legacy_verbs, as: nil
  match 'user/update(/:id)(.:format)', to: 'user#update', via: legacy_verbs, as: nil
end
