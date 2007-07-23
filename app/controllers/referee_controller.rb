class RefereeController < ApplicationController
  scaffold :referee
  before_filter :login_required, :except => [ :index, :list, :show ]
end
