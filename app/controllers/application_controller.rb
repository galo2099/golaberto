# Filters added to this controller will be run for all controllers in the application.
# Likewise, all the methods added will be available for all controllers.
require 'authenticated_system'
class ApplicationController < ActionController::Base
  include Pagy::Backend
  include AuthenticatedSystem
  include Userstamp

  before_action :set_locale
  before_action :set_current_user
  before_action :update_last_login

  check_authorization
  protect_from_forgery

  rescue_from CanCan::AccessDenied do |exception|
    flash[:notice] = _("Access denied!")
    redirect_to root_url
  end
  rescue_from ActiveRecord::RecordNotFound, :with => :record_not_found

  def cookie_timezone
    if not @timezone then
      timezone_str = cookies[:timezone] || MaxmindGeoIP2.locate(request.remote_ip).try(:fetch, "time_zone")
      timezone_str = ActiveSupport::TimeZone::MAPPING.key(timezone_str) || timezone_str
      @timezone = Time.find_zone(timezone_str) || Time.zone
    end
    @timezone
  end
  helper_method :cookie_timezone

  private
  def record_not_found
    render :text => "404 Not Found", :status => 404
  end

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

  def set_locale
    I18n.locale = extract_locale_from_tld || I18n.default_locale
  end
 
  # Get locale from top-level domain or return nil if such locale is not available
  def extract_locale_from_tld
    case request.host.split('.').last
    when "br"
      :"pt-BR"
    else
      :"en-US"
    end
  end
end
