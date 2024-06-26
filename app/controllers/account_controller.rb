class AccountController < ApplicationController
  N_("Account")

  skip_before_action :verify_authenticity_token, only: [:login]
  skip_authorization_check
  before_action :login_from_cookie

  def login
    @user = User.new(params.fetch(:user, {}).permit(:login, :password))
    if using_open_id?
      open_id_authentication
    elsif not @user.login.blank?
      password_authentication
    else
      session[:return_to] = request.env["HTTP_REFERER"]
    end
  end

  def signup
    @user = User.new(params.fetch(:user, {}).permit(:login, :password, :email, :password_confirmation))
    return unless request.post?
    @user.save!
    self.current_user = @user
    redirect_back_or_default(:controller => :home)
    flash[:notice] = _("Thanks for signing up!")
  rescue ActiveRecord::RecordInvalid
    render :action => 'signup'
  end

  def google_onetap
    if g_csrf_token_valid?
      payload = Google::Auth::IDTokens.verify_oidc(params[:credential], aud: Rails.application.credentials.google_api[:client_id])
      self.current_user = User.find_or_create_by!(openid_connect_token: payload['sub']) do |user|
        user.name = payload['name']
        user.avatar_remote_url = payload['picture']
        user.email = payload['email']
      end
      redirect_back(fallback_location: root_path)
    else
      redirect_to(root_path, notice: _("sign in failed"))
    end
  end

  def google_signin
    auth = request.env["omniauth.auth"]

    self.current_user = User.where(openid_connect_token: auth[:uid]).first
    if not self.current_user
    #  self.current_user = User.where(identity_url: auth[:extra][:id_info][:openid_id]).first
    #  if self.current_user
    #    self.current_user.openid_connect_token = auth[:uid]
    #  else
        self.current_user = User.create(openid_connect_token: auth[:uid], name: auth[:info][:name], avatar_remote_url: auth[:info][:image])
    #  end
    end
    successful_login
  end

  def failure
    failed_login _("Username or password invalid")
    redirect_to(controller: :account, action: :login)
  end

  def logout
    self.current_user.forget_me if logged_in?
    cookies.delete :auth_token
    reset_session
    flash[:notice] = _("You have been logged out.")
    redirect_back(fallback_location: root_path)
  end

  private

  def password_authentication
    self.current_user = User.authenticate(@user.login, @user.password)
    if logged_in?
      successful_login
    else
      failed_login _("Username or password invalid")
    end
  end

  def open_id_authentication
    @user.identity_url = params[:openid_url]
    authenticate_with_open_id do |result, identity_url|
      if result.successful?
        if self.current_user = User.find_or_create_by(identity_url: identity_url)
          successful_login
        else
          failed_login _("Sorry, no user by that identity URL exists (%{identity_url})" % { :identity_url => identity_url })
        end
      else
        failed_login result.message
      end
    end
  rescue
    failed_login $!.to_s
  end

  def successful_login
    if params[:remember_me] == "1"
      self.current_user.remember_me
      cookies[:auth_token] = { :value => self.current_user.remember_token , :expires => self.current_user.remember_token_expires_at }
    end
    current_user.last_login = Time.now
    current_user.save!
    redirect_to(:controller => :user, :action => :show, :id => current_user.id)
    flash[:notice] = _("Logged in successfully")
  end

  def failed_login(message)
    flash[:notice] = message
  end

  def g_csrf_token_valid?
    cookies['g_csrf_token'] == params['g_csrf_token']
  end
end
