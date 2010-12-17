set :user, "galo_2099"
set :domain, "golaberto.com.br"
set :application, "golaberto"
set :rake, "/home/galo_2099/.gem/ruby/1.8/bin/rake"

set :deploy_to, "/home/galo_2099/#{application}"

# Dreamhost doesn't allow you to use sudo.
set :use_sudo, false

set :scm, :git
set :local_scm_command, "git"
set :repository, "git://github.com/galo2099/golaberto.git"
set :deploy_via, :remote_cache
set :branch, "master"

ssh_options[:paranoid] = false
ssh_options[:encryption] = [ "aes128-cbc", "aes256-cbc", "3des-cbc" ]
ssh_options[:auth_methods] = [ "publickey", "keyboard-interactive", "password" ]

role :app, domain
role :web, domain
role :db,  domain, :primary => true

after "deploy:update_code", :roles => :app do
  stats = <<-JS
<script type='text/javascript'>
var gaJsHost = (('https:' == document.location.protocol) ? 'https://ssl.' : 'http://www.');
document.write(unescape('%3Cscript src=\\"' + gaJsHost + 'google-analytics.com/ga.js\\" type=\\"text/javascript\\"%3E%3C/script%3E'));
</script>
<script type='text/javascript'>
try {
var pageTracker = _gat._getTracker('UA-1911106-4');
pageTracker._trackPageview();
} catch(err) {}</script>
  JS
  layout = "#{release_path}/app/views/layouts/application.rhtml"
  run "sed -i \"s^</body>^#{stats}</body>^\" #{layout}"

  run "sed -i -e 's/development/production/' #{release_path}/lib/daemons/delayed_jobs.rb"

  run "patch #{release_path}/config/environment.rb -i #{shared_path}/config/environment.patch"

  run "cp -f #{shared_path}/config/database.yml #{release_path}/config/database.yml"

  run "ln -s #{shared_path}/countries #{release_path}/public/images/"
  run "ln -s #{shared_path}/logos #{release_path}/public/images/"
  run "ln -s #{shared_path}/users #{release_path}/public/images/"

  run "#{rake} -f #{release_path}/Rakefile asset:packager:build_all"
end

namespace :deploy do
  desc "Restart the FCGI processes on the app server as a regular user."
  task :restart, :roles => :app do
    run "touch #{current_path}/tmp/restart.txt"
  end
end
