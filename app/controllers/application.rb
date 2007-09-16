# Filters added to this controller will be run for all controllers in the application.
# Likewise, all the methods added will be available for all controllers.
class ApplicationController < ActionController::Base
  include AuthenticatedSystem
  helper :date
  before_filter do |c|
    User.current_user =
        User.find(c.session[:user]) unless c.session[:user].nil?
  end
end
