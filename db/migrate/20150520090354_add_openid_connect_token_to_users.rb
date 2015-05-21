class AddOpenidConnectTokenToUsers < ActiveRecord::Migration
  def change
    add_column :users, :openid_connect_token, :string
  end
end
