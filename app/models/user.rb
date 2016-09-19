require 'digest/sha1'
class User < ActiveRecord::Base
  cattr_accessor :current_user
  model_stamper

  has_attached_file :avatar,
      styles: lambda { |attachment|
        options = { format: "png", filter_background: attachment.instance.filter_image_background? }
        { medium: options.merge(geometry: "100x100"),
          thumb: options.merge(geometry: "15x15") }
      },
      processors: [ :logo ],
      storage: :s3,
      s3_region: 'us-east-1',
      s3_credentials: Rails.application.secrets.s3,
      s3_headers: { 'Cache-Control' => 'max-age=315576000', 'Expires' => 10.years.from_now.httpdate },
      default_url: 'https://s3.amazonaws.com/:bucket/:style.png',
      path: ":class/:attachment/:id/:style.:extension"
  validates_attachment :avatar, content_type: { content_type: ["image/jpg", "image/jpeg", "image/png", "image/gif"] }

  # Virtual attribute to see if we should filter the image background
  attr_accessor :filter_image_background

  # Virtual attribute for the unencrypted password
  attr_accessor :password
  N_("User|Password")

  has_and_belongs_to_many :roles
  has_many :comments, -> { order('created_at ASC') }, :dependent => :destroy
  has_many :game_edits, :class_name => "Game::Version", :foreign_key => "updater_id"

  validates_presence_of     :login, :email, :if => :not_openid?
  validates_presence_of     :password,                   :if => :password_required?
  validates_presence_of     :password_confirmation,      :if => :password_required?
  validates_length_of       :password, :within => 4..40, :if => :password_required?
  validates_confirmation_of :password,                   :if => :password_required?
  validates_length_of       :login,    :within => 3..40, :if => :not_openid?
  validates_length_of       :email,    :within => 3..100, :if => :not_openid?
  validates_uniqueness_of   :login, :email, :case_sensitive => false, :allow_nil => true, :if => :not_openid?
  validates_length_of       :name, :maximum => 30, :allow_blank => true
  validates_length_of       :location, :maximum => 100, :allow_blank => true
  validates_length_of       :about_me, :maximum => 2000, :allow_blank => true

  before_save :encrypt_password
  after_create :create_logo
  after_create :add_initial_roles

  # Authenticates a user by their login name and unencrypted password.  Returns the user or nil.
  def self.authenticate(login, password)
    u = find_by_login(login) # need to get the salt
    u && u.authenticated?(password) ? u : nil
  end

  # Encrypts some data with the salt.
  def self.encrypt(password, salt)
    Digest::SHA1.hexdigest("--#{salt}--#{password}--")
  end

  def filter_image_background?
    return filter_image_background == "1"
  end

  # has_role? simply needs to return true or false whether a user has a role or not.
  def has_role?(role_in_question)
    @_list ||= self.roles.collect(&:name)
    (@_list.include?(role_in_question.to_s) )
  end

  # Encrypts the password with the user salt
  def encrypt(password)
    self.class.encrypt(password, salt)
  end

  def authenticated?(password)
    crypted_password == encrypt(password)
  end

  def remember_token?
    remember_token_expires_at && Time.now.utc < remember_token_expires_at
  end

  # These create and unset the fields required for remembering users between browser closes
  def remember_me
    self.remember_token_expires_at = 2.weeks.from_now.utc
    self.remember_token            = encrypt("#{email}--#{remember_token_expires_at}")
    save(:validate => false)
  end

  def forget_me
    self.remember_token_expires_at = nil
    self.remember_token            = nil
    save(:validate => false)
  end

  def display_login
    login or identity_url
  end

  def display_name
    name.blank? ? display_login : name
  end

  protected
    # before filter
    def encrypt_password
      return if password.blank?
      self.salt = Digest::SHA1.hexdigest("--#{Time.now.to_s}--#{login}--") if new_record?
      self.crypted_password = encrypt(password)
    end

    def password_required?
      not_openid? && (crypted_password.blank? or not password.blank?)
    end

    def not_openid?
      identity_url.blank? && openid_connect_token.blank?
    end

    def create_logo
      tmpfile = Tempfile.new("user_#{id}_icon")
      begin
        icon = Quilt::Identicon.new display_login, :size => 100
        tmpfile.binmode
        tmpfile.write icon.to_blob
        tmpfile.flush
        self.avatar = tmpfile
        save!
      ensure
        tmpfile.close
        tmpfile.unlink
      end
    end

    def add_initial_roles
      self.roles << Role.find_or_create_by(name: "commenter")
    end
end
