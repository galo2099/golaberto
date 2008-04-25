set :user, "galo_2099"
set :application, "golaberto"
set :domain, "golaberto.com.br"

set :deploy_to, "/home/galo_2099/#{domain}"

# Dreamhost doesn't allow you to use sudo.
set :use_sudo, false

set :scm, :git
set :git, "/home/galo_2099/git/bin/git"
set :repository,  "git://github.com/galo2099/golaberto.git"
set :deploy_via, :remote_cache
set :branch, "master"

ssh_options[:paranoid] = false

role :app, domain
role :web, domain
role :db,  domain, :primary => true

task :after_symlink, :roles => :app do
  stats = <<-JS
    <script src="http://www.google-analytics.com/urchin.js">
    </script>
    <script type="text/javascript">
    _uacct = "UA-1911106-4";
    urchinTracker();
    </script>
  JS
  layout = "#{current_path}/app/views/layouts/application.html"
  run "sed -i 's?</body>?#{stats}</body>?' #{layout}"
end

