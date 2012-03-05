Golaberto::Application.routes.draw do
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
  root :to => "home#index"

  # map championship actions
  match 'championship/show/:id/phases/:phase' => 'championship#phases'
  match 'championship/show/:id/phases/:phase/team_json/' => 'championship#team_json'
  match 'championship/show/:id/games/:phase' => 'championship#games'
  match 'championship/show/:id/new_game/:phase' => 'championship#new_game'
  match 'championship/show/:id/team/:team' => 'championship#team'

  match 'game/list(/cat/:category)(/p/:page)' => 'game#list', :constraints => { :page => /\d+/ }, :defaults => { :type => :scheduled, :page => 1, :category => 1 }
  match 'game/list/played(/cat/:category)(/p/:page)' => 'game#list', :constraints => { :page => /\d+/ }, :defaults => { :type => :played, :page => 1, :category => 1 }

  match 'team/games/scheduled/:id(/cat/:category)(/p/:page)' => 'team#games', :defaults => { :played => false, :category => 1, :page => 1 }
  match 'team/games/played/:id(/cat/:category)(/p/:page)' => 'team#games', :defaults => { :played => true, :category => 1, :page => 1 }

  # Install the default route as the lowest priority.
  match ':controller(/:action(/:id))(.:format)'
end
