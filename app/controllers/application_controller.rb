# Filters added to this controller will be run for all controllers in the application.
# Likewise, all the methods added will be available for all controllers.
require 'authenticated_system'
class ApplicationController < ActionController::Base
  include AuthenticatedSystem
  include Userstamp

  before_filter :set_gettext_locale
  before_filter :set_current_user
  before_filter :update_last_login

  check_authorization
  protect_from_forgery

  rescue_from CanCan::AccessDenied do |exception|
    flash[:notice] = _("Access denied!")
    redirect_to root_url
  end

  private
  def set_current_user
    User.current_user = current_user
  end

  def update_last_login
    if logged_in?
      if not current_user.last_login or Time.now - current_user.last_login > 1.hour
        current_user.last_login = Time.now
        current_user.save!
      end
    end
  end
end
