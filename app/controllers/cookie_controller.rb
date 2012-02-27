class CookieController < ApplicationController
  N_("Cookie")

 def set_cookie
   code = params[:id]
   cookies[:locale] =
     {
      :value => code,
      :expires => Time.now + 1.year,
      :path => '/'
     }
   session[:locale] = code
   redirect_to :back
   rescue ActionController::RedirectBackError
   redirect_to "/"
 end
end
