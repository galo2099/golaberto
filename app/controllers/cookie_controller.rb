class CookieController < ApplicationController
 def set_cookie
   code = params[:id]
   cookies[:lang] =
     {
      :value => code,
      :expires => Time.now + 1.year,
      :path => '/'
     }
   redirect_to :back
   rescue ActionController::RedirectBackError
   redirect_to "/"
 end
end
