set :application, 'golaberto'
set :repo_url, 'git://github.com/galo2099/golaberto.git'
set :branch, "rails4"

set :deploy_to, '/home/robsonbraga/golaberto'

set :linked_files, %w{config/database.yml config/secrets.yml}
set :linked_dirs, %w{bin log tmp/pids tmp/cache tmp/sockets vendor/bundle public/system}

namespace :deploy do
    desc 'Restart application'
    task :restart do
      on roles(:app), in: :sequence, wait: 5 do
        execute :touch, release_path.join('tmp/restart.txt')
      end
    end

    after :publishing, 'deploy:restart'
    after :finishing, 'deploy:cleanup'
end

namespace :deploy do
#  desc "precompile and deploy the assets to the server"
#  after "deploy:update_code", "deploy:precompile_assets"
#  task :precompile_assets, :roles => :app do
#    run_locally "#{rake} assets:clean && #{rake} RAILS_ENV=#{rails_env} RAILS_GROUPS=assets assets:precompile"
#    run "mkdir -p #{release_path}/public/assets"
#    transfer(:up, "public/assets", "#{release_path}/public/assets") { print "." }
#  end
#  after "deploy:update_code", "deploy:set_image_symlinks"
#  task :set_image_symlinks, :roles => :app do
#    run "mkdir -p #{release_path}/public/images"
#    run "ln -s #{shared_path}/images/countries #{release_path}/public/images/"
#    run "ln -s #{shared_path}/images/logos #{release_path}/public/images/"
#    run "ln -s #{shared_path}/images/users #{release_path}/public/images/"
#  end
  after "deploy:update_code", "deploy:install_google_analytics"
  task :install_google_analytics, :roles => :app do
    stats = <<-JS
      <script type="text/javascript">
  (function(i,s,o,g,r,a,m){i["GoogleAnalyticsObject"]=r;i[r]=i[r]||function(){
  (i[r].q=i[r].q||[]).push(arguments)},i[r].l=1*new Date();a=s.createElement(o),
  m=s.getElementsByTagName(o)[0];a.async=1;a.src=g;m.parentNode.insertBefore(a,m)
  })(window,document,"script","//www.google-analytics.com/analytics.js","ga");

  ga("create", "UA-1911106-4", "golaberto.com.br", {"siteSpeedSampleRate": 100});
  ga("send", "pageview");
      </script>
    JS
    layout = "#{release_path}/app/views/layouts/application.html.erb"
    run "sed -i 's#</body>##{stats}</body>#' #{layout}"
  end
end
