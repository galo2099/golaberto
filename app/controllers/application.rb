# Filters added to this controller will be run for all controllers in the application.
# Likewise, all the methods added will be available for all controllers.
class ApplicationController < ActionController::Base
  require 'gettext_date'
  init_gettext "golaberto"
  include AuthenticatedSystem

  include ExceptionNotifiable
  ExceptionNotifier.exception_recipients = %w(golaberto@gmail.com)
  ExceptionNotifier.sender_address =
    %("Application Error" <app.error@golaberto.com.br>)
  ExceptionNotifier.email_prefix = "[GolAberto] "

  helper :date
  before_filter do |c|
    User.current_user =
        User.find(c.session[:user]) unless c.session[:user].nil?
  end
end
