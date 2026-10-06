set :application, 'golaberto'
set :repo_url, 'git://github.com/galo2099/golaberto.git'
set :branch, "rails4"

set :deploy_to, '/home/robsonbraga/golaberto'

set :linked_files, %w{config/database.yml config/secrets.yml}
set :linked_dirs, %w{bin log tmp/pids tmp/cache tmp/sockets vendor/bundle public/system}

namespace :deploy do
  desc 'Build the SofaScore API fetcher'
  task :build_sofascore_fetch do
    on roles(:app) do
      within release_path do
        execute :cargo, :build, '--release', '--locked', '--manifest-path', 'sofascore-fetch-rust/Cargo.toml', '--target-dir', 'sofascore-fetch-rust/target'
        execute :install, '-m', '755', 'sofascore-fetch-rust/target/release/sofascore_fetch', 'bin/sofascore_fetch'
      end
    end
  end

  desc 'Restart application'
  task :restart do
    on roles(:app), in: :sequence, wait: 5 do
      execute :touch, release_path.join('tmp/restart.txt')
    end
  end

  task :install_google_analytics do
    stats = '<script type="text/javascript">(function(i,s,o,g,r,a,m){i["GoogleAnalyticsObject"]=r;i[r]=i[r]||function(){(i[r].q=i[r].q||[]).push(arguments)},i[r].l=1*new Date();a=s.createElement(o),m=s.getElementsByTagName(o)[0];a.async=1;a.src=g;m.parentNode.insertBefore(a,m)})(window,document,"script","//www.google-analytics.com/analytics.js","ga");ga("create", "UA-1911106-4", "golaberto.com.br", {"siteSpeedSampleRate": 100});ga("send", "pageview");</script>'
    layout = "#{release_path}/app/views/layouts/application.html.erb"
    on roles(:app) do
      execute :sed, "-i 's#</body>##{stats}</body>#' #{layout}"
    end
  end

  after :publishing, 'deploy:restart'
  before 'deploy:restart', 'deploy:build_sofascore_fetch'
  before 'deploy:restart', 'deploy:install_google_analytics'
  after :finishing, 'deploy:cleanup'
end
